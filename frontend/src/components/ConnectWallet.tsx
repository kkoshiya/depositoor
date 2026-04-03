import { useWallet } from './WalletProvider'
import type { WalletProviderDetail } from '../lib/wallets'
import './ConnectWallet.css'

function WalletList({ providers, connect }: {
  providers: WalletProviderDetail[]
  connect: (detail: WalletProviderDetail) => Promise<void>
}) {
  return (
    <div className="connect-wallet-list">
      {providers.map((detail) => (
        <button
          key={detail.info.uuid}
          className="wallet-option"
          onClick={() => connect(detail)}
        >
          <img
            src={detail.info.icon}
            alt={detail.info.name}
            className="wallet-option-icon"
          />
          <span className="wallet-option-name">{detail.info.name}</span>
        </button>
      ))}
    </div>
  )
}

export function ConnectWallet() {
  const { providers, connect } = useWallet()

  const webWallets = providers.filter((p) => p.source !== 'eip6963')
  const extensionWallets = providers.filter((p) => p.source === 'eip6963')

  return (
    <div className="connect-wallet">
      {webWallets.length > 0 && (
        <div className="connect-wallet-section">
          <div className="connect-wallet-label">Web wallets</div>
          <WalletList providers={webWallets} connect={connect} />
        </div>
      )}
      {extensionWallets.length > 0 && (
        <div className="connect-wallet-section">
          <div className="connect-wallet-label">Extension wallets</div>
          <WalletList providers={extensionWallets} connect={connect} />
        </div>
      )}
      {providers.length === 0 && (
        <div className="connect-wallet-empty">
          No wallets detected.
        </div>
      )}
    </div>
  )
}
