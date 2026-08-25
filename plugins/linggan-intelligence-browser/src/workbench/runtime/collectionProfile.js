const PROFILE_BY_TASK_TYPE = Object.freeze({
  'xhs.authorNoteLinks': 'author_links',
  'xhs.author_links': 'author_links',
  'xhs.collectAuthor': 'author_profile',
  'xhs.author_profile': 'author_profile',
  'xhs.list_scan': 'list_scan',
  'xhs.note_full': 'note_full',
  'xhs.comment_scan': 'comment_probe',
  'douyin.author_profile': 'author_profile',
  'douyin.list_scan': 'list_scan',
  'douyin.note_full': 'note_full',
  'douyin.comment_scan': 'comment_probe',
});

export function resolveCollectionProfile(task = {}, persistedContext = {}) {
  const explicitProfile = String(
    task?.collectionProfile
    || persistedContext?.collectionProfile
    || task?.payload?.collectionProfile
    || '',
  ).trim();
  if (explicitProfile) return explicitProfile;

  return PROFILE_BY_TASK_TYPE[String(task?.taskType || '').trim()] || '';
}
