import { Navigate, createBrowserRouter } from 'react-router-dom'

import { AppLayout } from '../layouts/AppLayout'
import { appRoutes, DEFAULT_ROUTE_PATH } from './routeConfig'

export const appRouter = createBrowserRouter([
  {
    path: '/',
    element: <Navigate to={DEFAULT_ROUTE_PATH} replace />,
  },
  {
    path: '/app',
    element: <AppLayout />,
    children: [
      {
        index: true,
        element: <Navigate to={DEFAULT_ROUTE_PATH} replace />,
      },
      ...appRoutes.map((route) => ({
        path: route.path.replace('/app/', ''),
        element: route.element,
      })),
    ],
  },
])
