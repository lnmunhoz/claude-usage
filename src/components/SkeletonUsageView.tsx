export function SkeletonUsageView() {
  return (
    <div className="panel">
      <div className="panel-header">
        <div className="skeleton skeleton-logo" />
        <div className="skeleton skeleton-badge" />
      </div>

      <div className="panel-bars">
        <div className="panel-bar-group">
          <div className="panel-bar-label-row">
            <div className="skeleton skeleton-text-sm" style={{ width: 100 }} />
            <div className="skeleton skeleton-text-sm" style={{ width: 50 }} />
          </div>
          <div className="skeleton skeleton-bar-track" />
          <span className="skeleton skeleton-text-xs" style={{ width: 80 }} />
        </div>

        <div className="panel-bar-group">
          <div className="panel-bar-label-row">
            <div className="skeleton skeleton-text-sm" style={{ width: 56 }} />
            <div className="skeleton skeleton-text-sm" style={{ width: 50 }} />
          </div>
          <div className="skeleton skeleton-bar-track" />
          <span className="skeleton skeleton-text-xs" style={{ width: 80 }} />
        </div>
      </div>

      <div className="skeleton skeleton-button" />
      <div className="skeleton skeleton-button" />
    </div>
  );
}
