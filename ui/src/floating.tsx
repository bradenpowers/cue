import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './app.css'
import Floating from './components/Floating'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Floating />
  </StrictMode>,
)
