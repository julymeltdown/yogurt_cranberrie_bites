import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { FoundationApp } from './app/FoundationApp'
import './shared/styles/base.css'

const root = document.getElementById('root')

if (root === null) {
  throw new Error('MCR Concert World requires a root element.')
}

createRoot(root).render(
  <StrictMode>
    <FoundationApp />
  </StrictMode>,
)
