import { ethers } from "hardhat"

type DeployArgs = {
  pasteId?: string
  contentHash?: string
  expiresAt?: number
  retentionClass?: number
  attestationRef?: string
}

async function main() {
  const args = parseArgs()
  const factory = await ethers.getContractFactory("PasteAnchor")
  const contract = await factory.deploy()
  await contract.deployed()

  console.log(`PasteAnchor deployed to ${contract.address}`)

  if (args.pasteId && args.contentHash) {
    const tx = await contract.anchorPaste(
      ethers.utils.arrayify(args.pasteId),
      ethers.utils.arrayify(args.contentHash),
      args.expiresAt ?? 0,
      args.retentionClass ?? 0,
      args.attestationRef ? ethers.utils.arrayify(args.attestationRef) : ethers.constants.HashZero
    )
    await tx.wait()
    console.log(`Anchored paste ${args.pasteId} (tx: ${tx.hash})`)
  }
}

function parseArgs(): DeployArgs {
  const argv = process.argv.slice(2)
  const args: DeployArgs = {}

  for (const part of argv) {
    const [key, value] = part.split("=")
    switch (key) {
      case "pasteId":
        args.pasteId = value
        break
      case "contentHash":
        args.contentHash = value
        break
      case "expiresAt":
        args.expiresAt = Number(value)
        break
      case "retentionClass":
        args.retentionClass = Number(value)
        break
      case "attestationRef":
        args.attestationRef = value
        break
      default:
        break
    }
  }

  return args
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
