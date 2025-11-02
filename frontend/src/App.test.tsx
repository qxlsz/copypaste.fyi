import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import '@testing-library/jest-dom/vitest'

import App from "./App"

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
})

const renderApp = () => {
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <App />
      </MemoryRouter>
    </QueryClientProvider>
  )
}

describe('App', () => {
  it('renders without crashing', () => {
    expect(() => renderApp()).not.toThrow()
  })

  it('renders the basic layout elements', () => {
    renderApp()
    
    // Check that the header with branding is present
    expect(screen.getByText('copypaste.fyi')).toBeInTheDocument()
  })
})
