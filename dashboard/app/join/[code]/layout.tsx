import type { ReactNode } from 'react';
import { DeckSplash } from '@/components/DeckSplash';

export default async function JoinLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ code: string }>;
}) {
  const { code } = await params;
  return (
    <>
      <DeckSplash sessionKey={`join:${code}`} />
      {children}
    </>
  );
}
