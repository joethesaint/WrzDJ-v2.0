'use client';

import { useEffect } from 'react';
import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { useAuth } from '@/lib/auth';
import { HairlineDeck } from '@/components/HairlineDeck';

export default function Home() {
  const { isAuthenticated, isLoading } = useAuth();
  const router = useRouter();

  useEffect(() => {
    if (!isLoading && isAuthenticated) {
      router.push('/dashboard');
    }
  }, [isAuthenticated, isLoading, router]);

  if (isLoading || isAuthenticated) {
    return (
      <div className="container">
        <div className="loading">Loading...</div>
      </div>
    );
  }

  // Copy and layout are provisional until the landing page is designed.
  return (
    <main
      style={{
        minHeight: '100vh',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: '1.5rem',
        padding: '2rem 1rem',
        textAlign: 'center',
        colorScheme: 'dark',
        ['--hairline-plate' as string]: 'var(--bg, #0a0a0a)',
      }}
    >
      <div style={{ width: 'min(90vw, 480px)' }}>
        <HairlineDeck />
      </div>
      <h1 style={{ fontSize: '2rem', margin: 0 }}>WrzDJ</h1>
      <p style={{ margin: 0, opacity: 0.7, maxWidth: '28rem' }}>
        Song requests from the crowd, straight to the decks.
      </p>
      <Link href="/login" className="btn btn-primary">
        DJ login
      </Link>
    </main>
  );
}
