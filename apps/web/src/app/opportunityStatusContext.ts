import { createContext, useContext } from 'react';

export interface OpportunityStatus {
  state: 'enabled' | 'disabled' | 'loading' | 'unavailable';
  retry: () => void;
}

export const OpportunityStatusContext = createContext<OpportunityStatus>({ state: 'disabled', retry: () => {} });
export const useOpportunityStatus = () => useContext(OpportunityStatusContext);
