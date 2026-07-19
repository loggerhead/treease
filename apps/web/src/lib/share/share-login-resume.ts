const shareLoginResumeKey = 'treease:resume-share-after-login';

export type SessionStorageLike = Pick<Storage, 'getItem' | 'removeItem' | 'setItem'>;

export function requestShareResume(storage: SessionStorageLike): void {
  storage.setItem(shareLoginResumeKey, '1');
}

export function consumeShareResume(storage: SessionStorageLike): boolean {
  if (storage.getItem(shareLoginResumeKey) !== '1') return false;
  storage.removeItem(shareLoginResumeKey);
  return true;
}
