import { Porto, Chains } from 'porto'
import { createCoinbaseWalletSDK } from '@coinbase/wallet-sdk'
import type { WalletProviderDetail, EIP1193Provider } from './wallets'

// ---------------------------------------------------------------------------
// Placeholder icons (will be replaced with real ones later)
// ---------------------------------------------------------------------------

const PORTO_ICON =
  'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><circle cx="50" cy="50" r="45" fill="%23000"/><text x="50" y="67" text-anchor="middle" fill="white" font-size="50" font-family="sans-serif">P</text></svg>'

const COINBASE_ICON =
  'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><circle cx="50" cy="50" r="45" fill="%230052FF"/><text x="50" y="67" text-anchor="middle" fill="white" font-size="50" font-family="sans-serif">C</text></svg>'

// ---------------------------------------------------------------------------
// Adapter: wrap any object with request/on/removeListener into our minimal
// EIP1193Provider interface so TypeScript is happy without pulling in heavy
// generics from each SDK.
// ---------------------------------------------------------------------------

function toEIP1193(raw: unknown): EIP1193Provider {
  const p = raw as {
    request(args: { method: string; params?: unknown[] }): Promise<unknown>
    on(event: string, handler: (...args: unknown[]) => void): void
    removeListener(event: string, handler: (...args: unknown[]) => void): void
  }
  return {
    request: (args) => p.request(args),
    on: (event, handler) => p.on(event, handler),
    removeListener: (event, handler) => p.removeListener(event, handler),
  }
}

// ---------------------------------------------------------------------------
// Porto Smart Wallet
// ---------------------------------------------------------------------------

export function createPortoProvider(): WalletProviderDetail {
  const porto = Porto.create({
    announceProvider: false,
    chains: [
      Chains.mainnet,
      Chains.base,
      Chains.arbitrum,
      Chains.optimism,
      Chains.polygon,
      Chains.bsc,
    ],
  })

  return {
    info: {
      uuid: 'porto-smart-wallet',
      name: 'Porto',
      icon: PORTO_ICON,
      rdns: 'xyz.ithaca.porto',
    },
    provider: toEIP1193(porto.provider),
    source: 'porto',
  }
}

// ---------------------------------------------------------------------------
// Coinbase Smart Wallet
// ---------------------------------------------------------------------------

export function createCoinbaseSmartWalletProvider(): WalletProviderDetail {
  const sdk = createCoinbaseWalletSDK({
    appName: 'Depositoor',
    appChainIds: [8453, 1, 42161, 10, 137, 56],
    preference: { options: 'smartWalletOnly' },
  })

  return {
    info: {
      uuid: 'coinbase-smart-wallet',
      name: 'Coinbase Smart Wallet',
      icon: COINBASE_ICON,
      rdns: 'com.coinbase.wallet',
    },
    provider: toEIP1193(sdk.getProvider()),
    source: 'coinbase-smart-wallet',
  }
}
