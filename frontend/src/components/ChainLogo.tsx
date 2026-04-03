import type { Chain } from '../lib/constants'

type Props = {
  chain: Chain
  size?: number
}

export function ChainLogo({ chain, size = 18 }: Props) {
  const r = Math.round(size * 0.22)
  const scale = chain.logoScale ?? 0.75
  const imgSize = Math.round(size * scale)

  return (
    <div
      style={{
        width: size,
        height: size,
        borderRadius: r,
        background: chain.logoBg ?? chain.color,
        overflow: 'hidden',
        flexShrink: 0,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <img
        src={`/chain-logos/${chain.logo}.svg`}
        alt={chain.name}
        width={imgSize}
        height={imgSize}
        style={{ display: 'block' }}
      />
    </div>
  )
}
