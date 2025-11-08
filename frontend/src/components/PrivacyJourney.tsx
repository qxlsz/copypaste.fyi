import { useEffect, useState } from 'react'
import { Shield, Lock, Eye, Globe, Server, Zap } from 'lucide-react'

interface JourneyStep {
  icon: React.ComponentType<{ className?: string }>
  label: string
  detail: string
  detected: boolean
}

export const PrivacyJourney = () => {
  const [isExpanded, setIsExpanded] = useState(false)
  const [steps, setSteps] = useState<JourneyStep[]>([])

  useEffect(() => {
    const detectPrivacyFeatures = async () => {
      const journeySteps: JourneyStep[] = []

      // Check for HTTPS
      const isHttps = window.location.protocol === 'https:'
      journeySteps.push({
        icon: Lock,
        label: 'Encrypted Connection',
        detail: isHttps ? 'TLS/SSL encryption active' : 'Unencrypted connection',
        detected: isHttps,
      })

      // Check for Tor (onion address)
      const isTor = window.location.hostname.endsWith('.onion')
      journeySteps.push({
        icon: Globe,
        label: 'Tor Network',
        detail: isTor ? 'Accessing via Tor onion service' : 'Direct connection',
        detected: isTor,
      })

      // Check for VPN/Proxy indicators (basic heuristics)
      let isVpnLikely = false
      try {
        const response = await fetch('https://ipapi.co/json/', { signal: AbortSignal.timeout(3000) })
        const data = await response.json()
        isVpnLikely = data.org?.toLowerCase().includes('vpn') || 
                      data.org?.toLowerCase().includes('proxy') ||
                      data.asn?.toString().includes('VPN')
      } catch {
        // Ignore errors
      }
      journeySteps.push({
        icon: Shield,
        label: 'VPN/Proxy',
        detail: isVpnLikely ? 'Possible VPN/proxy detected' : 'Direct IP connection',
        detected: isVpnLikely,
      })

      // Check for Do Not Track
      const dnt = navigator.doNotTrack === '1' || (window as Window & { doNotTrack?: string }).doNotTrack === '1'
      journeySteps.push({
        icon: Eye,
        label: 'Do Not Track',
        detail: dnt ? 'DNT header enabled' : 'DNT not set',
        detected: dnt,
      })

      // Check for Private/Incognito mode (heuristic)
      let isPrivateMode = false
      try {
        // Test for private mode using storage quota
        if ('storage' in navigator && 'estimate' in navigator.storage) {
          const { quota } = await navigator.storage.estimate()
          isPrivateMode = (quota || 0) < 120000000 // Less than 120MB suggests private mode
        }
      } catch {
        // Some browsers block this in private mode
        isPrivateMode = true
      }
      journeySteps.push({
        icon: Zap,
        label: 'Private Browsing',
        detail: isPrivateMode ? 'Likely in private/incognito mode' : 'Normal browsing mode',
        detected: isPrivateMode,
      })

      // Client-side encryption
      journeySteps.push({
        icon: Server,
        label: 'Client-Side Encryption',
        detail: 'Keys never leave your device',
        detected: true, // Always true for this app
      })

      setSteps(journeySteps)
    }

    detectPrivacyFeatures()
  }, [])

  const privacyScore = steps.filter(s => s.detected).length
  const totalSteps = steps.length

  return (
    <div className="fixed bottom-4 left-4 z-50 sm:bottom-6 sm:left-6">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="group relative flex items-center gap-1.5 rounded-full bg-gradient-to-r from-primary/90 to-primary/70 px-3 py-1.5 text-xs font-semibold text-white shadow-lg shadow-primary/30 backdrop-blur-sm transition-all hover:shadow-xl hover:shadow-primary/40 sm:gap-2 sm:px-4 sm:py-2 sm:text-sm"
        aria-label="Privacy journey details"
      >
        <Shield className="h-3.5 w-3.5 sm:h-4 sm:w-4" />
        <span className="hidden sm:inline">Privacy:</span>
        <span className="font-mono">{privacyScore}/{totalSteps}</span>
        <div className="absolute -top-0.5 -right-0.5 h-2 w-2 animate-pulse rounded-full bg-green-400 sm:-top-1 sm:-right-1 sm:h-3 sm:w-3" />
      </button>

      {isExpanded && (
        <div className="absolute bottom-12 left-0 w-[calc(100vw-2rem)] max-w-sm rounded-2xl border border-slate-200 bg-white/95 p-3 shadow-2xl backdrop-blur-md dark:border-slate-700 dark:bg-slate-900/95 sm:bottom-14 sm:w-80 sm:p-4">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
              Your Privacy Journey
            </h3>
            <button
              onClick={() => setIsExpanded(false)}
              className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
              aria-label="Close"
            >
              ✕
            </button>
          </div>

          <p className="mb-4 text-xs text-slate-600 dark:text-slate-400">
            We detected {privacyScore} privacy measure{privacyScore !== 1 ? 's' : ''} protecting your connection
          </p>

          <div className="space-y-2">
            {steps.map((step, index) => {
              const Icon = step.icon
              return (
                <div
                  key={index}
                  className={`flex items-start gap-3 rounded-lg p-2 transition ${
                    step.detected
                      ? 'bg-green-50 dark:bg-green-900/20'
                      : 'bg-slate-50 dark:bg-slate-800/50'
                  }`}
                >
                  <Icon
                    className={`mt-0.5 h-4 w-4 flex-shrink-0 ${
                      step.detected
                        ? 'text-green-600 dark:text-green-400'
                        : 'text-slate-400 dark:text-slate-600'
                    }`}
                  />
                  <div className="flex-1 min-w-0">
                    <p
                      className={`text-xs font-semibold ${
                        step.detected
                          ? 'text-green-900 dark:text-green-100'
                          : 'text-slate-700 dark:text-slate-300'
                      }`}
                    >
                      {step.label}
                    </p>
                    <p className="text-xs text-slate-600 dark:text-slate-400">
                      {step.detail}
                    </p>
                  </div>
                  {step.detected && (
                    <span className="text-green-600 dark:text-green-400">✓</span>
                  )}
                </div>
              )
            })}
          </div>

          <div className="mt-4 rounded-lg bg-primary/10 p-3 text-xs text-primary">
            <p className="font-semibold">🔒 Privacy First</p>
            <p className="mt-1 text-xs opacity-90">
              All encryption happens in your browser. Your keys never touch our servers.
            </p>
          </div>

          <a
            href="https://how-did-i-get-here.net/"
            target="_blank"
            rel="noopener noreferrer"
            className="mt-3 block text-center text-xs text-slate-500 hover:text-primary dark:text-slate-400"
          >
            Inspired by how-did-i-get-here.net ↗
          </a>
        </div>
      )}
    </div>
  )
}
