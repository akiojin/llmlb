import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { TooltipProvider } from '@/components/ui/tooltip'
import { Toaster } from '@/components/ui/toaster'
import ResetPasswordPage from './pages/ResetPassword'
import './index.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <TooltipProvider>
      <ResetPasswordPage />
      <Toaster />
    </TooltipProvider>
  </StrictMode>
)
