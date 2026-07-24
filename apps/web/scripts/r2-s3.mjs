import { createHash, createHmac } from 'node:crypto';

const SERVICE = 's3';
const REGION = 'auto';

export function createR2S3Client({ accountId, accessKeyId, secretAccessKey, bucket }) {
  const endpoint = `https://${accountId}.r2.cloudflarestorage.com`;

  return {
    async headObject(key) {
      const response = await request({
        endpoint,
        accessKeyId,
        secretAccessKey,
        method: 'HEAD',
        bucket,
        key,
      });
      if (response.status === 404) return null;
      if (!response.ok) throw await responseError('HEAD', key, response);
      return response.headers.get('x-amz-meta-treease-sha256');
    },

    async putObject({ key, body, contentType, cacheControl, sha256 }) {
      const response = await request({
        endpoint,
        accessKeyId,
        secretAccessKey,
        method: 'PUT',
        bucket,
        key,
        body,
        headers: {
          'cache-control': cacheControl,
          'content-type': contentType,
          'x-amz-meta-treease-sha256': sha256,
        },
      });
      if (!response.ok) throw await responseError('PUT', key, response);
    },
  };
}

async function request({ endpoint, accessKeyId, secretAccessKey, method, bucket, key, body, headers = {} }) {
  const url = `${endpoint}/${encodePathSegment(bucket)}/${key.split('/').map(encodePathSegment).join('/')}`;
  const payloadHash = body === undefined ? sha256('') : sha256(body);
  const amzDate = new Date().toISOString().replace(/[-:]|\.\d{3}/g, '');
  const date = amzDate.slice(0, 8);
  const requestHeaders = {
    host: new URL(url).host,
    'x-amz-content-sha256': payloadHash,
    'x-amz-date': amzDate,
    ...headers,
  };
  const normalizedHeaders = normalizeHeaders(requestHeaders);
  const signedHeaders = Object.keys(normalizedHeaders).sort().join(';');
  const canonicalHeaders = Object.entries(normalizedHeaders)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, value]) => `${name}:${value}\n`)
    .join('');
  const canonicalRequest = [
    method,
    new URL(url).pathname,
    '',
    canonicalHeaders,
    signedHeaders,
    payloadHash,
  ].join('\n');
  const scope = `${date}/${REGION}/${SERVICE}/aws4_request`;
  const signingKey = hmac(hmac(hmac(hmac(`AWS4${secretAccessKey}`, date), REGION), SERVICE), 'aws4_request');
  const signature = hmac(signingKey, `AWS4-HMAC-SHA256\n${amzDate}\n${scope}\n${sha256(canonicalRequest)}`).toString('hex');
  const authorization = `AWS4-HMAC-SHA256 Credential=${accessKeyId}/${scope}, SignedHeaders=${signedHeaders}, Signature=${signature}`;

  return fetch(url, {
    method,
    headers: { ...requestHeaders, authorization },
    body,
  });
}

function normalizeHeaders(headers) {
  return Object.fromEntries(Object.entries(headers).map(([name, value]) => [
    name.toLowerCase(), String(value).trim().replace(/\s+/g, ' '),
  ]));
}

function encodePathSegment(value) {
  return encodeURIComponent(value);
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function hmac(key, value) {
  return createHmac('sha256', key).update(value).digest();
}

async function responseError(method, key, response) {
  const details = (await response.text()).trim().replace(/\s+/g, ' ').slice(0, 500);
  return new Error(`R2 ${method} ${key} failed with ${response.status}${details ? `: ${details}` : ''}`);
}
