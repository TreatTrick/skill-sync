import mitt from 'mitt'

export interface AppEvents {
  [key: string]: unknown
  [key: symbol]: unknown
  notification: {
    message: string
  }
}

export const appEventBus = mitt<AppEvents>()
