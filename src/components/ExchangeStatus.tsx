export function ExchangeStatus({
  code,
  error,
  completed = true,
}: {
  code: number | null;
  error?: string | null;
  completed?: boolean;
}) {
  if (!completed) return <span className="status-code status-pending">In flight</span>;
  return (
    <span
      className={`status-code ${error || (code && code >= 400) ? 'status-error' : code && code >= 300 ? 'status-redirect' : 'status-ok'}`}
    >
      {error ? 'ERR' : (code ?? 'N/A')}
    </span>
  );
}
