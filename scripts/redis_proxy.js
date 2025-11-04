#!/usr/bin/env node
/* eslint-disable no-console */
const http = require('http')
const net = require('net')
const { URL } = require('url')

const listenPort = parseInt(process.env.REDIS_HTTP_PORT ?? '8787', 10)
const redisUrl = new URL(process.env.REDIS_TCP_URL ?? 'redis://127.0.0.1:6380')

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

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, `http://${req.headers.host}`)
    const segments = url.pathname.split('/').filter(Boolean)

    if (segments.length === 0) {
      res.writeHead(404)
      res.end()
      return
    }

    const command = segments[0].toLowerCase()
    const key = segments[1]

    if (!key) {
      res.writeHead(400)
      res.end(JSON.stringify({ error: 'Missing key' }))
      return
    }

    if (command === 'set') {
      const value = decodeURIComponent(segments.slice(2).join('/'))
      await sendCommand(['SET', key, value])
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ result: 'OK', error: null }))
      return
    }

    if (command === 'setex') {
      const ttl = segments[2]
      const value = decodeURIComponent(segments.slice(3).join('/'))
      await sendCommand(['SETEX', key, ttl, value])
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ result: 'OK', error: null }))
      return
    }

    if (command === 'get') {
      const result = await sendCommand(['GET', key])
      if (result == null) {
        res.writeHead(404)
        res.end()
        return
      }
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ result, error: null }))
      return
    }

    if (command === 'del') {
      await sendCommand(['DEL', key])
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ result: 'OK', error: null }))
      return
    }

    res.writeHead(404)
    res.end()
  } catch (error) {
    console.error('Proxy error:', error)
    res.writeHead(500, { 'Content-Type': 'application/json' })
    res.end(JSON.stringify({ error: error.message }))
  }
})

server.listen(listenPort, () => {
  console.log(`Redis REST proxy listening on http://127.0.0.1:${listenPort}`)
})
