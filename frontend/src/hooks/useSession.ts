import { useEffect, useRef, useState, useCallback } from 'react'
import { API_URL } from '../lib/constants'
import type { SignedAuthJson } from '../App'

export type SessionStatus = 'idle' | 'registering' | 'pending' | 'detected' | 'sweeping' | 'swept' | 'failed' | 'expired'

export type Session = {
  id: string
  expiresAt: number
  status: SessionStatus
}

type RegisterRequest = {
  burner_address: string
  eip7702_auth: SignedAuthJson
  destination_address: string
  destination_chain: number
}

type RegisterResponse = {
  id: string
  expires_at: number
  status: string
}

export function useSession(
  burnerAddress: string | null,
  signedAuth: SignedAuthJson | null,
  destinationAddress: string,
  destinationChainId: number,
) {
  const [session, setSession] = useState<Session | null>(null)
  const [error, setError] = useState<string | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const register = useCallback(async () => {
    if (!burnerAddress || !signedAuth || !destinationAddress) return

    setError(null)
    setSession(prev => prev ? { ...prev, status: 'registering' } : null)

    try {
      const body: RegisterRequest = {
        burner_address: burnerAddress,
        eip7702_auth: signedAuth,
        destination_address: destinationAddress,
        destination_chain: destinationChainId,
      }

      const res = await fetch(`${API_URL}/sessions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })

      if (!res.ok) {
        const text = await res.text()
        throw new Error(`Registration failed: ${text}`)
      }

      const data: RegisterResponse = await res.json()

      setSession({
        id: data.id,
        expiresAt: data.expires_at,
        status: data.status as SessionStatus,
      })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unknown error')
      setSession(null)
    }
  }, [burnerAddress, signedAuth, destinationAddress, destinationChainId])

  // Auto-register when destination is configured
  useEffect(() => {
    if (burnerAddress && signedAuth && destinationAddress && !session) {
      register()
    }
  }, [burnerAddress, signedAuth, destinationAddress, session, register])

  // Expiry timer — auto-renew when session expires
  useEffect(() => {
    if (!session || !session.expiresAt) return
    if (session.status === 'swept' || session.status === 'failed') return

    const msUntilExpiry = session.expiresAt * 1000 - Date.now()
    if (msUntilExpiry <= 0) {
      // Already expired, renew now
      setSession(null) // triggers re-register via the effect above
      return
    }

    timerRef.current = setTimeout(() => {
      setSession(null) // triggers re-register
    }, msUntilExpiry)

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [session])

  return { session, error, register }
}
