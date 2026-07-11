import { Navigate, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import { DownloadOverlay } from "./DownloadOverlay";
import { PermissionOverlay } from "./PermissionOverlay";
import {
  AuthOverlay,
  ConfirmOverlay,
  FindOverlay,
} from "./MoreOverlays";
import { overlayBackdropVariants } from "../motion";

const OVERLAYS = {
  download: DownloadOverlay,
  permission: PermissionOverlay,
  confirm: ConfirmOverlay,
  auth: AuthOverlay,
  find: FindOverlay,
} as const;

export type OverlayId = keyof typeof OVERLAYS;

export function isOverlayId(value: string | undefined): value is OverlayId {
  return !!value && value in OVERLAYS;
}

/** Transparent CEF popup host — `#/overlay/{id}?…` */
export function OverlayPage() {
  const { kind } = useParams();

  if (!isOverlayId(kind)) {
    return <Navigate to="/overlay/permission" replace />;
  }

  const Cmp = OVERLAYS[kind];

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={kind}
        className="h-full w-full"
        variants={overlayBackdropVariants}
        initial="initial"
        animate="animate"
        exit="exit"
      >
        <Cmp />
      </motion.div>
    </AnimatePresence>
  );
}
