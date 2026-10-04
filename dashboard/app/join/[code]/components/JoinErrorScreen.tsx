import { ExpiredStage, ExpiredStamp } from './ExpiredStage';

interface JoinErrorScreenProps {
  error: { message: string; status: number } | null;
}

const HINT = 'At the venue? Ask the DJ for the current QR code.';

/** What a guest sees when the join link can't open an event. */
export function JoinErrorScreen({ error }: JoinErrorScreenProps) {
  const is410 = error?.status === 410;
  const is404 = error?.status === 404;
  // The API answers 410 for both expired and archived events; the detail says which.
  const archived = is410 && /archived/i.test(error?.message ?? '');

  const title = archived
    ? 'Event Closed'
    : is410
      ? 'Event Expired'
      : is404
        ? 'Event Not Found'
        : 'Oops!';
  const body = archived
    ? 'The DJ has closed this event.'
    : is410
      ? 'This event has ended and is no longer accepting requests.'
      : is404
        ? 'This event does not exist.'
        : error?.message || 'Event not found or has expired.';

  return (
    <div
      className="guest-tower"
      style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '2rem' }}
    >
      <div style={{ textAlign: 'center', maxWidth: is410 ? 420 : 360 }}>
        {is410 && <ExpiredStage />}
        <h1
          style={{
            fontFamily: 'inherit',
            fontSize: 33.9,
            fontWeight: 800,
            letterSpacing: -0.6,
            margin: '0 0 10px',
          }}
        >
          {title}
        </h1>
        <div style={{ color: 'rgba(255,255,255,0.5)', fontSize: 18.2 }}>{body}</div>
        {is410 && (
          <div style={{ color: 'rgba(255,255,255,0.5)', fontSize: 15, marginTop: 16 }}>{HINT}</div>
        )}
        {is410 && <ExpiredStamp label={archived ? 'Closed' : 'Ended'} />}
      </div>
    </div>
  );
}
