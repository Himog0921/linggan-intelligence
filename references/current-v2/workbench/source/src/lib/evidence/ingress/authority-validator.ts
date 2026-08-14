/**
 * EvidenceIngressAuthorityValidator per 07-evidence-ingress-release-b-contract.md §3.4.
 *
 * The dark core injects this via constructor — no built-in fake production
 * authorization. B1-B-02 only uses test validators; production validators are
 * assembled when real session or authorization references exist in the
 * corresponding caller adapters.
 */

import type {
  EvidenceIngressAuthorityV2,
  EvidenceIngressRejectReason,
  CaptureHeaderV2,
} from "./types";

export type AuthorityValidationResult =
  | { valid: true }
  | { valid: false; reason: EvidenceIngressRejectReason };

/**
 * Injectable authority validator. Each implementation proves that the
 * authority context is genuine for its ingressKind.
 *
 * B1-B-10 provides production-shaped dark validators for execution,
 * manual_import, and recovery. None has a runtime caller until the separately
 * authorized one-time Release-B cutover; migration remains test-only.
 */
export interface EvidenceIngressAuthorityValidator {
  validate(
    authority: EvidenceIngressAuthorityV2,
    header: CaptureHeaderV2
  ): Promise<AuthorityValidationResult> | AuthorityValidationResult;
}
