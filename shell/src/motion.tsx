import { type ReactNode } from "react";
import { motion, AnimatePresence, type Variants } from "framer-motion";
import { useLocation } from "react-router-dom";

export const pageVariants: Variants = {
  initial: { opacity: 0 },
  animate: {
    opacity: 1,
    transition: { duration: 0.2, ease: [0.22, 1, 0.36, 1] },
  },
  exit: {
    opacity: 0,
    transition: { duration: 0.12, ease: "easeIn" },
  },
};

export const overlayBackdropVariants: Variants = {
  initial: { opacity: 0 },
  animate: { opacity: 1, transition: { duration: 0.12 } },
  exit: { opacity: 0, transition: { duration: 0.1 } },
};

export const overlayCardVariants: Variants = {
  initial: { opacity: 0, y: 8 },
  animate: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.16, ease: "easeOut" },
  },
  exit: {
    opacity: 0,
    y: 4,
    transition: { duration: 0.1 },
  },
};

export const listContainerVariants: Variants = {
  animate: {},
};

export const listItemVariants: Variants = {
  initial: { opacity: 1 },
  animate: { opacity: 1 },
};

export function PageTransition({ children }: { children: ReactNode }) {
  const location = useLocation();
  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={location.pathname}
        className="h-full min-h-0 overflow-hidden"
        variants={pageVariants}
        initial="initial"
        animate="animate"
        exit="exit"
      >
        {children}
      </motion.div>
    </AnimatePresence>
  );
}
