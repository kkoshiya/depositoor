export const IMPL_ADDRESS = '0x000000000000000000000000000000000000dEaD' as const

export const API_URL = import.meta.env.VITE_API_URL ?? 'http://localhost:3001'

export type Chain = {
  id: number
  name: string
  color: string
  cctpDomain: number
}

export const SUPPORTED_CHAINS: Chain[] = [
  { id: 42161, name: 'Arbitrum', color: '#12AAFF', cctpDomain: 3 },
  { id: 8453,  name: 'Base',     color: '#0052FF', cctpDomain: 6 },
  { id: 10,    name: 'Optimism', color: '#FF0420', cctpDomain: 2 },
]
