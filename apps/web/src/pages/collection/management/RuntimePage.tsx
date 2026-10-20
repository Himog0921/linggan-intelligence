import { runtimeDestination } from './runtimeDestination';
import { Navigate, useSearchParams } from 'react-router';
export { RuntimeContent } from './RuntimeContent';

export function RuntimePage() {
  const [params] = useSearchParams();
  return <Navigate replace to={runtimeDestination(params)}/>;
}
