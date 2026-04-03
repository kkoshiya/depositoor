import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react'
import {
  discoverProviders,
  type WalletProviderDetail,
  type EIP1193Provider,
} from '../lib/wallets'
import { createPortoProvider, createCoinbaseSmartWalletProvider } from '../lib/smart-wallets'

interface WalletState {
  address: string | null
  chainId: number | null
  provider: EIP1193Provider | null
  providers: WalletProviderDetail[]
  connect: (detail: WalletProviderDetail) => Promise<void>
  disconnect: () => void
  switchChain: (chainId: number) => Promise<void>
}

const WalletContext = createContext<WalletState | null>(null)

export function useWallet() {
  const ctx = useContext(WalletContext)
  if (!ctx) throw new Error('useWallet must be used within WalletProvider')
  return ctx
}

export function WalletProvider({ children }: { children: ReactNode }) {
  const [providers, setProviders] = useState<WalletProviderDetail[]>([])
  const [provider, setProvider] = useState<EIP1193Provider | null>(null)
  const [address, setAddress] = useState<string | null>(null)
  const [chainId, setChainId] = useState<number | null>(null)

  // Discover EIP-6963 providers on mount
  useEffect(() => {
    // Initialize with static smart wallet providers
    const smartWallets = [
      createPortoProvider(),
      createCoinbaseSmartWalletProvider(),
    ]
    setProviders(smartWallets)

    // Discover browser extension providers via EIP-6963
    return discoverProviders((detail) => {
      setProviders((prev) => {
        if (prev.some((p) => p.info.uuid === detail.info.uuid)) return prev
        return [...prev, detail]
      })
    })
  }, [])

  // Listen for chain/account changes on the active provider
  useEffect(() => {
    if (!provider) return

    const onChainChanged = (raw: unknown) => {
      setChainId(Number(raw as string))
    }
    const onAccountsChanged = (raw: unknown) => {
      const accounts = raw as string[]
      if (accounts.length === 0) {
        setAddress(null)
        setProvider(null)
        setChainId(null)
      } else {
        setAddress(accounts[0])
      }
    }

    provider.on('chainChanged', onChainChanged)
    provider.on('accountsChanged', onAccountsChanged)
    return () => {
      provider.removeListener('chainChanged', onChainChanged)
      provider.removeListener('accountsChanged', onAccountsChanged)
    }
  }, [provider])

  const connect = useCallback(async (detail: WalletProviderDetail) => {
    const accounts = (await detail.provider.request({
      method: 'eth_requestAccounts',
    })) as string[]
    const chain = (await detail.provider.request({
      method: 'eth_chainId',
    })) as string

    setProvider(detail.provider)
    setAddress(accounts[0])
    setChainId(Number(chain))
  }, [])

  const disconnect = useCallback(() => {
    setProvider(null)
    setAddress(null)
    setChainId(null)
  }, [])

  const switchChain = useCallback(
    async (targetChainId: number) => {
      if (!provider) return
      await provider.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: '0x' + targetChainId.toString(16) }],
      })
    },
    [provider]
  )

  return (
    <WalletContext.Provider
      value={{ address, chainId, provider, providers, connect, disconnect, switchChain }}
    >
      {children}
    </WalletContext.Provider>
  )
}
