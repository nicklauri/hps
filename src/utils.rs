use std::{
    mem::MaybeUninit,
    ops::{Range, RangeTo},
};

use anyhow::{bail, Context, Result};
use httparse::{Header, Request, Response, Status};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpStream, ToSocketAddrs},
};

use crate::config::MAX_NUMBERS_OF_HEADERS;

pub trait RequestResponseExt {
    fn get_content_length(&self) -> Result<Option<usize>>;
}

impl<'header, 'buf> RequestResponseExt for Request<'header, 'buf> {
    #[inline]
    fn get_content_length(&self) -> Result<Option<usize>> {
        get_content_length(self.headers)
    }
}

impl<'header, 'buf> RequestResponseExt for Response<'header, 'buf> {
    #[inline]
    fn get_content_length(&self) -> Result<Option<usize>> {
        get_content_length(self.headers)
    }
}

#[inline]
fn get_content_length(headers: &[Header]) -> Result<Option<usize>> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("content-length"))
        .map(|h| {
            std::str::from_utf8(h.value)
                .with_context(|| format!("\"Content-Length\" value is not valid UTF-8, value={:?}", h.value))
                .and_then(|s| {
                    str::parse::<usize>(s)
                        .with_context(|| format!("\"Content-Length\" value is not valid number, value={:?}", h.value))
                })
        })
        .transpose()
}

pub(crate) trait RangeExt {
    fn range_from_part(&self, part: Self) -> Range<usize>;
    fn from_range(&self, range: Range<usize>) -> Option<Self>
    where
        Self: Sized;
}

impl RangeExt for &[u8] {
    #[inline]
    fn range_from_part(&self, part: Self) -> Range<usize> {
        let src_start = self.as_ptr() as usize;
        let part_start = part.as_ptr() as usize;

        Range {
            start: part_start - src_start,
            end: part_start - src_start + part.len(),
        }
    }

    #[inline]
    fn from_range(&self, range: Range<usize>) -> Option<Self>
    where
        Self: Sized,
    {
        self.get(range)
    }
}

impl RangeExt for &str {
    #[inline]
    fn range_from_part(&self, part: &str) -> Range<usize> {
        self.as_bytes().range_from_part(part.as_bytes())
    }

    #[inline]
    fn from_range(&self, range: Range<usize>) -> Option<Self>
    where
        Self: Sized,
    {
        self.as_bytes()
            .from_range(range)
            .map(std::str::from_utf8)
            .and_then(std::result::Result::ok)
    }
}

#[inline]
pub fn create_uninit_headers<'a>() -> [MaybeUninit<Header<'a>>; MAX_NUMBERS_OF_HEADERS] {
    unsafe { MaybeUninit::uninit().assume_init() }
}

#[inline]
fn create_headers<'a>() -> [Header<'a>; MAX_NUMBERS_OF_HEADERS] {
    unsafe { MaybeUninit::uninit().assume_init() }
}

#[inline]
pub async fn connect(addr: impl ToSocketAddrs) -> Result<TcpStream> {
    Ok(TcpStream::connect(addr).await?)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ParseHttpData<'a> {
    pub parsed_len: usize,
    pub content_length: usize,
    pub path: Option<&'a str>,
}

// impl<'a> ParseHttpData<'a> {
//     fn new() -> Self {
//         Self {
//             parsed_len: 0,
//             content_length: 0,
//             path: "",
//         }
//     }
// }

#[inline]
pub fn parse_request<'a>(buf: &'a [u8]) -> Result<Option<ParseHttpData<'a>>> {
    let mut request = Request::new(&mut []);
    let mut headers = create_uninit_headers();

    let parse_result = request.parse_with_uninit_headers(buf, &mut headers);

    let result = handle_parse_result(parse_result, request.path, request);

    result
}

#[inline]
pub fn parse_response<'a>(buf: &'a [u8]) -> Result<Option<ParseHttpData<'a>>> {
    let mut headers = create_headers();
    let mut response = Response::new(&mut headers);

    let parse_result = response.parse(buf);

    handle_parse_result(parse_result, None, response)
}

#[inline]
fn handle_parse_result<'a>(
    parse_result: httparse::Result<usize>,
    path: Option<&'a str>,
    reqres: impl RequestResponseExt,
) -> Result<Option<ParseHttpData<'a>>> {
    let mut parsed_data = ParseHttpData::default();

    parsed_data.path = path.filter(|p| !p.is_empty());

    parsed_data.parsed_len = match parse_result {
        Ok(Status::Complete(parsed_len)) => parsed_len,
        Ok(Status::Partial) => return Ok(None),
        Err(err) => {
            bail!("parse header error: {err}");
        }
    };

    parsed_data.content_length = reqres.get_content_length()?.unwrap_or(0);

    Ok(Some(parsed_data))
}

#[inline]
pub async fn copy_nbuf<'a, R, W>(reader: &mut R, writer: &mut W, buf: &mut [u8], mut size: usize) -> Result<()>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    fn get_buf_range(buf_len: usize, max: usize) -> RangeTo<usize> {
        if buf_len > max {
            ..max
        } else {
            ..buf_len
        }
    }

    while size > 0 {
        let buf_range = get_buf_range(size, buf.len());

        let amount = reader.read(&mut buf[buf_range]).await?;

        if amount == 0 {
            bail!(size);
        }

        writer.write_all(&buf[..amount]).await?;

        size -= amount;
    }

    Ok(())
}
