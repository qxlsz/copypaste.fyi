import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { BrowserRouter, Route, Routes } from 'react-router-dom'

import { Layout } from './components/Layout'
import { PasteFormPage } from './pages/PasteForm'
import { StatsPage } from './pages/Stats'

function App() {
  const [queryClient] = useState(() => new QueryClient())

  return (
    <BrowserRouter>
      <QueryClientProvider client={queryClient}>
        <Routes>
          <Route path="/" element={<Layout />}>
            <Route index element={<PasteFormPage />} />
            <Route path="stats" element={<StatsPage />} />
          </Route>
        </Routes>
      </QueryClientProvider>
    </BrowserRouter>
  )
}

export default App
