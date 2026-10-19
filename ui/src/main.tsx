import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './app.css'
import Workspace from './components/Workspace'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Workspace />
  </StrictMode>,
)
