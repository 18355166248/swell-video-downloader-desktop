import type { PropsWithChildren } from 'react';
import { Provider } from '@react-spectrum/s2';

export function AppProviders({ children }: PropsWithChildren) {
  return (
    <Provider locale="zh-CN" colorScheme="light" background="base">
      {children}
    </Provider>
  );
}
