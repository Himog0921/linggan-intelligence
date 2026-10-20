export function runtimeDestination(params: URLSearchParams) {
  const requested = params.get('return_to') ?? '';
  const safe = /^\/collection\/(targets|tasks)(?:[?#]|$)/.test(requested) && !/[\\\r\n]/.test(requested);
  const destination = new URL(safe ? requested : '/collection/targets', 'http://collection.local');
  if (!safe && params.has('domain')) destination.searchParams.set('domain',params.get('domain')!);
  destination.searchParams.set('runtime','1');
  if(params.has('station')) destination.searchParams.set('station',params.get('station')!);
  return `${destination.pathname}${destination.search}${destination.hash}`;
}
