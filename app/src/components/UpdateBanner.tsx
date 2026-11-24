// A bar at the top of the page while an update is on its way or waiting to be downloaded by hand.

import { ArrowSquareOutIcon, DownloadSimpleIcon, WarningIcon, XIcon } from "@phosphor-icons/react";
import { send, useStore } from "../store";
import "./UpdateBanner.css";

export default function UpdateBanner() {
  const u = useStore((s) => s.sections.update);
  // without the git hash
  const app = useStore((s) => s.sections.catalog?.appVersion?.split(" ")[0]);
  if (!u) return null;

  let text: string;
  let action: string | null = null;
  let progress: number | null = null;
  switch (u.kind) {
    case "available":
      text = `Open Stomp Manager ${u.version} is out (you have ${app ?? "an older version"}).`;
      action = "Get it";
      break;
    case "downloading":
      text = `Downloading Open Stomp Manager ${u.version}…`;
      progress = u.percent;
      break;
    case "ready":
      text = `Open Stomp Manager ${u.version} installs and restarts when the pedal work in progress is done.`;
      break;
    case "installing":
      text = `Installing Open Stomp Manager ${u.version}…`;
      break;
    case "failed":
      text = u.message;
      action = "Releases";
      break;
    default:
      return null;
  }
  const failed = u.kind === "failed";
  const dismissable = u.kind === "available" || failed;

  return (
    <div className={`update-banner ${failed ? "update-failed" : ""}`} role="status">
      {failed ? <WarningIcon size={16} weight="fill" /> : <DownloadSimpleIcon size={16} weight="bold" />}
      <span className="update-text">{text}</span>
      {progress != null && (
        <span className="update-bar">
          <span style={{ width: `${progress}%` }} />
        </span>
      )}
      {action && (
        <button className="pill pill-main" onClick={() => send({ type: "openRelease" })}>
          <ArrowSquareOutIcon size={14} /> {action}
        </button>
      )}
      {dismissable && (
        <button className="update-close" title="Hide" onClick={() => send({ type: "dismissUpdate" })}>
          <XIcon size={14} />
        </button>
      )}
    </div>
  );
}
