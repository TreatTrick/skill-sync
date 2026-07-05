import { RouterProvider } from 'react-router-dom'

import { AppProviders } from './app/providers/AppProviders'
import { appRouter } from './app/router/AppRouter'

const App = () => (
  <AppProviders>
    <RouterProvider router={appRouter} />
  </AppProviders>
)

export default App
