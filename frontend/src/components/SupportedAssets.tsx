import { useState, useRef, useEffect } from 'react'
import { SUPPORTED_CHAINS, SUPPORTED_TOKENS } from '../lib/constants'
import { ChainLogo } from './ChainLogo'
import './SupportedAssets.css'

const ACTIVE_CHAINS = SUPPORTED_CHAINS.filter(c => !c.comingSoon)

type Props = {
  initialChainId?: number
}

export function SupportedAssets({ initialChainId }: Props) {
  const [selectedChainId, setSelectedChainId] = useState(
    initialChainId && ACTIVE_CHAINS.some(c => c.id === initialChainId)
      ? initialChainId
      : ACTIVE_CHAINS[0].id
  )
  const selectedChain = ACTIVE_CHAINS.find(c => c.id === selectedChainId) ?? ACTIVE_CHAINS[0]
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const [copiedAddr, setCopiedAddr] = useState<string | null>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [])

  const copyAddr = async (addr: string) => {
    await navigator.clipboard.writeText(addr)
    setCopiedAddr(addr)
    setTimeout(() => setCopiedAddr(null), 1500)
  }

  const truncate = (addr: string) =>
    `${addr.slice(0, 6)}...${addr.slice(-4)}`

  return (
    <div className="supported-assets">
      <div className="chain-dropdown" ref={dropdownRef}>
        <button
          className="chain-dropdown-trigger"
          onClick={() => setDropdownOpen(o => !o)}
          type="button"
        >
          <ChainLogo chain={selectedChain} size={20} />
          <span className="chain-dropdown-label">{selectedChain.name}</span>
          <svg
            className={`chain-dropdown-chevron ${dropdownOpen ? 'is-open' : ''}`}
            width="14" height="14" viewBox="0 0 24 24"
            fill="none" stroke="currentColor" strokeWidth="2"
            strokeLinecap="round" strokeLinejoin="round"
          >
            <polyline points="6 9 12 15 18 9" />
          </svg>
        </button>
        {dropdownOpen && (
          <div className="chain-dropdown-menu">
            {ACTIVE_CHAINS.map(chain => (
              <button
                key={chain.id}
                className={`chain-dropdown-option ${chain.id === selectedChainId ? 'active' : ''}`}
                onClick={() => { setSelectedChainId(chain.id); setDropdownOpen(false) }}
                type="button"
              >
                <ChainLogo chain={chain} size={20} />
                <span>{chain.name}</span>
                {chain.id === selectedChainId && (
                  <svg className="chain-dropdown-check" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                )}
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="token-list">
        {SUPPORTED_TOKENS.map(token => {
          const addr = token.addresses[selectedChainId]
          if (!addr) return null
          const isCopied = copiedAddr === addr
          return (
            <button
              key={token.symbol}
              className="token-row"
              onClick={() => copyAddr(addr)}
              type="button"
            >
              <svg className="token-icon" width="32" height="32" viewBox="0 0 32 32">
                <circle cx="16" cy="16" r="16" fill={token.color} />
                <text
                  x="16" y="17"
                  textAnchor="middle"
                  dominantBaseline="central"
                  fill="white"
                  fontSize="11"
                  fontWeight="700"
                  fontFamily="var(--font)"
                >
                  {token.symbol[0]}
                </text>
              </svg>
              <div className="token-info">
                <span className="token-symbol">{token.symbol}</span>
                <span className="token-name">{token.name}</span>
              </div>
              <div className="token-addr-group">
                <span className="token-addr mono">{truncate(addr)}</span>
                <span className={`token-copy-icon ${isCopied ? 'is-copied' : ''}`}>
                  {isCopied ? (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  ) : (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                    </svg>
                  )}
                </span>
              </div>
            </button>
          )
        })}
      </div>
    </div>
  )
}
