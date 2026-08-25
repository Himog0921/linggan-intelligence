export function requireControlReceipt(result, expectedState) {
  if (result?.success && result?.state === expectedState) return result;
  throw new Error(result?.message || `页面没有确认${expectedState}当前任务`);
}
