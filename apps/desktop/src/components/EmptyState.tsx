import "./EmptyState.css";

interface EmptyStateProps {
  title: string;
  hint: string;
  actions?: React.ReactNode;
}

export function EmptyState({ title, hint, actions }: EmptyStateProps) {
  return (
    <div className="empty-state">
      <div className="empty-state-mark" aria-hidden>
        z6
      </div>
      <h2 className="empty-state-title">{title}</h2>
      <p className="empty-state-hint">{hint}</p>
      {actions && <div className="empty-state-actions">{actions}</div>}
    </div>
  );
}
