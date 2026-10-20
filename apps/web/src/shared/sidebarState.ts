import { createContext } from 'react';

export const SidebarState = createContext({ collapsed: false, toggle: () => {} });
