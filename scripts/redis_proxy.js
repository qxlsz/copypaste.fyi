#!/usr/bin/env node
/* eslint-disable no-console */
const http = require('http')
const net = require('net')
const { URL } = require('url')

const listenPort = parseInt(process.env.REDIS_HTTP_PORT ?? '8787', 10)
const listenHost = process.env.REDIS_HTTP_HOST ?? '127.0.0.1'
const redisUrl = new URL(process.env.REDIS_TCP_URL ?? 'redis://127.0.0.1:6380')
const maxRequestBytes = 4 * 1024 * 1024
const allowedArities = new Map([
  ['SET', 3],
  ['SETEX', 4],
  ['GET', 2],
  ['GETDEL', 2],
  ['DEL', 2],
])

function encodeRESP(args) {
  const parts = [`*${args.length}\r\n`]
  for (const arg of args) {
    const buf = Buffer.from(String(arg))
    parts.push(`$${buf.length}\r\n`)
    parts.push(buf)
    parts.push('\r\n')
  }
  return Buffer.concat(parts.map((part) => (typeof part === 'string' ? Buffer.from(part) : part)))
}

function parseRESP(buffer) {
  const type = buffer[0]
  const rest = buffer.slice(1).toString()
  if (type === 43) {
    // + Simple String
    return rest.trim()
  }
  if (type === 36) {
    // $ Bulk String
    if (rest.startsWith('-1')) {
      return null
    }
    const [, ...bodyParts] = rest.split('\r\n')
    return bodyParts[0] ?? null
  }
  if (type === 58) {
    // : Integer
    return parseInt(rest, 10)
  }
  if (type === 45) {
    // - Error
    throw new Error(rest.trim())
  }
  throw new Error(`Unsupported RESP response: ${buffer.toString()}`)
}

function sendCommand(args) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(
      {
        host: redisUrl.hostname,
        port: Number(redisUrl.port || 6379),
      },
      () => {
        socket.write(encodeRESP(args))
      }
    )

    let chunks = []
    socket.on('data', (chunk) => {
      chunks.push(chunk)
    })

    socket.on('end', () => {
      try {
        const buffer = Buffer.concat(chunks)
        const result = parseRESP(buffer)
        resolve(result)
      } catch (error) {
        reject(error)
      }
    })

    socket.on('error', (error) => {
      reject(error)
    })
  })
}

function readJsonCommand(req) {
  return new Promise((resolve, reject) => {
    const chunks = []
    let received = 0
    let rejected = false

    req.on('data', (chunk) => {
      if (rejected) return
      received += chunk.length
      if (received > maxRequestBytes) {
        rejected = true
        reject(Object.assign(new Error('request too large'), { statusCode: 413 }))
        return
      }
      chunks.push(chunk)
    })
    req.on('end', () => {
      if (rejected) return
      try {
        const command = JSON.parse(Buffer.concat(chunks).toString('utf8'))
        if (!Array.isArray(command) || command.some((part) => typeof part !== 'string')) {
          throw Object.assign(new Error('command must be an array of strings'), { statusCode: 400 })
        }
        const name = command[0]?.toUpperCase()
        if (!name || allowedArities.get(name) !== command.length) {
          throw Object.assign(new Error('unsupported command or arity'), { statusCode: 400 })
        }
        if (command[1].length === 0 || command[1].length > 512) {
          throw Object.assign(new Error('invalid key'), { statusCode: 400 })
        }
        command[0] = name
        resolve(command)
      } catch (error) {
        reject(error.statusCode ? error : Object.assign(error, { statusCode: 400 }))
      }
    })
    req.on('error', reject)
  })
}

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, `http://${req.headers.host}`)
    if (req.method !== 'POST' || url.pathname !== '/' || url.search || url.hash) {
      res.writeHead(404, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ error: 'Not found' }))
      return
    }
    const command = await readJsonCommand(req)
    const result = await sendCommand(command)
    res.writeHead(200, {
      'Content-Type': 'application/json',
      'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
    })
    res.end(JSON.stringify({ result, error: null }))
  } catch (error) {
    const statusCode = error.statusCode ?? 500
    console.error(`Redis REST proxy request failed (${statusCode})`)
    res.writeHead(statusCode, {
      'Content-Type': 'application/json',
      'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff',
    })
    res.end(JSON.stringify({ error: statusCode >= 500 ? 'Redis command failed' : error.message }))
  }
})

server.listen(listenPort, listenHost, () => {
  console.log(`Redis REST proxy listening on http://${listenHost}:${listenPort}`)
})
