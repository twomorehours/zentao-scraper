use anyhow::{Ok, Result};
use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

static SNAMP: Lazy<HashMap<i32, &str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(1, "3");
    m.insert(2, "4");
    m.insert(3, "5");
    m.insert(4, "7");
    m.insert(5, "6");
    m.insert(6, "2");
    m.insert(7, "8");
    m.insert(8, "9");
    m.insert(9, "1");
    m.insert(10, "10");
    m
});
static BUG_TR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<tr data-id.+?</tr>").unwrap());
static BUG_DETAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"data-id='(\d+)?'.+class='c-title text-left' title='([^']+)?'.+class='c-status bug-[a-z]+?' title='([^']+)?'.+class='c-assignedTo has-btn text-left.+?title='([^']+)?'").unwrap()
});
static MS_BUG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"【(MS)?[0-9]{6}】").unwrap());

#[derive(Debug, Deserialize, Serialize)]
pub struct Bug {
    id: i32,
    title: String,
    status: String,
    assignee: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnBugs {
    system_name: String,
    bugs: Vec<Bug>,
    active: i32,
    total: i32,
    ms_active: i32,
    ms_total: i32,
}

impl Bug {
    fn is_ms(&self) -> bool {
        MS_BUG_RE.is_match(&self.title)
    }

    fn is_active(&self) -> bool {
        self.status == "激活"
    }

    fn match_keyword(&self, keyword: &str) -> bool {
        self.id.to_string() == keyword
            || self.assignee.contains(keyword)
            || self.title.contains(keyword)
            || self.status == keyword
    }
}

impl SnBugs {
    pub fn active(&self) -> i32 {
        self.active
    }
    pub fn total(&self) -> i32 {
        self.total
    }
    pub fn ms_active(&self) -> i32 {
        self.ms_active
    }
    pub fn ms_total(&self) -> i32 {
        self.ms_total
    }
}

pub async fn get_sx(
    no: i32,
    http_client: &Client,
    verbose: bool,
    keyword: Option<String>,
) -> Result<SnBugs> {
    let url = format!(
        "http://10.170.136.67:8080/zentao/bug-browse-{}-all-unclosed-0--500-500-1.html?tid=0nkkhq04",
        SNAMP.get(&no).unwrap()
    );
    let html = http_client
        .get(&url)
        .send()
        .await?
        .text()
        .await?
        .replace('\n', " ");

    let mut bugs = Vec::new();
    for caps in BUG_TR_RE.captures_iter(&html) {
        let tr = caps
            .get(0)
            .ok_or(anyhow::anyhow!("re match error"))?
            .as_str();

        let caps = BUG_DETAIL_RE
            .captures(tr)
            .ok_or(anyhow::anyhow!("re match error"))?;
        let data_id = caps
            .get(1)
            .ok_or(anyhow::anyhow!("re match error"))?
            .as_str();
        let title = caps
            .get(2)
            .ok_or(anyhow::anyhow!("re match error"))?
            .as_str();
        let status = caps
            .get(3)
            .ok_or(anyhow::anyhow!("re match error"))?
            .as_str();
        let assignee = caps
            .get(4)
            .ok_or(anyhow::anyhow!("re match error"))?
            .as_str();
        let bug = Bug {
            id: data_id.parse().unwrap(),
            title: title.to_string(),
            status: status.to_string(),
            assignee: assignee.to_string(),
        };
        if let Some(keyword) = keyword.as_ref() {
            if !keyword.is_empty() && !bug.match_keyword(keyword) {
                continue;
            }
        }
        bugs.push(bug);
    }

    let total_count = bugs.len() as i32;
    let mut ms_bug_count = 0;
    let mut active_bug_count = 0;
    let mut active_ms_bug_count = 0;

    for bug in bugs.iter() {
        if bug.is_active() {
            active_bug_count += 1;
        }
        if bug.is_ms() {
            ms_bug_count += 1;
        }
        if bug.is_ms() && bug.is_active() {
            active_ms_bug_count += 1;
        }
    }
    if !verbose {
        bugs = vec![];
    }

    Ok(SnBugs {
        system_name: format!("S{no}"),
        total: total_count,
        bugs,
        active: active_bug_count,
        ms_active: active_ms_bug_count,
        ms_total: ms_bug_count,
    })
}

pub async fn login(username: &str, password: &str, client: &reqwest::Client) -> Result<()> {
    let r = client
        .get("http://10.170.136.67:8080/zentao/user-refreshRandom.html")
        .send()
        .await?
        .text()
        .await?;

    let password = md5_digest(&format!("{}{}", md5_digest(password), r));
    let mut params = HashMap::new();
    params.insert("account", username);
    params.insert("password", &password);
    params.insert("passwordStrength", "1");
    params.insert("referer", "/zentao/");
    params.insert("verifyRand", &r);
    params.insert("captcha", "");
    params.insert("keepLogin", "0");

    let resp: serde_json::Value = client
        .post("http://10.170.136.67:8080/zentao/user-login.html")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .header("Origin", "http://10.170.136.67:8080")
        .header("Proxy-Connection", "keep-alive")
        .header(
            "Referer",
            "http://10.170.136.67:8080/zentao/user-login.html",
        )
        .form(&params)
        .send()
        .await?
        .json()
        .await?;

    let result = resp.get("result").unwrap().as_str().unwrap_or("fail");
    if result == "fail" {
        return Err(anyhow::anyhow!("login failed"));
    }

    Ok(())
}

fn md5_digest(content: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..])
}

#[cfg(test)]
mod tests {
    use crate::{md5_digest, BUG_TR_RE, MS_BUG_RE};

    #[test]
    fn bug_re_match() {
        let trcontet = r#"<tr data-id='5848'>
        <td class='c-id cell-id'>
            <div class='checkbox-primary'><input type='checkbox' name='bugIDList[]' value='5848'  id='bugIDList5848' title=''/>
                <label for='bugIDList5848'></label></div><a
                href='/zentao/bug-view-5848.html' data-app='qa'>5848</a>
        </td>
        <td class='c-severity'>
            <span class='label-severity' data-severity='3' title='3'></span></td>
        <td class='c-pri'><span class='label-pri label-pri-3' title='3'>3</span></td>
        <td class='c-confirmed text-center'><span class='confirm0' title='否'>否</span>
        </td>
        <td class='c-title text-left' title='【S7】【量产】故障关联分析模型，全条件输入页面默认的数据，查询不到结果'><a
                href='/zentao/bug-view-5848.html' style='color: '
                data-app=qa>【S7】【量产】故障关联分析模型，全条件输入页面默认的数据，查询不到结果</a>
        </td>
        <td class='c-status bug-active' title='激活'>
            <span class='status-bug status-active'>激活</span></td>
        <td class='c-openedBy c-user' title='苏海龙'>苏海龙</td>
        <td class='c-openedDate'>10-26 10:50</td>
        <td class='c-assignedTo has-btn text-left'><a
                href='/zentao/bug-assignTo-5848.html?onlybody=yes'
                class='iframe btn btn-icon-left btn-sm '><i class='icon icon-hand-right'></i>
                <span title='刘利利' class='text-primary'>刘利利</span></a>
        </td>
        <td class='c-resolution'></td>
        <td class='c-actions'><a href='/zentao/bug-confirmBug-5848.html?onlybody=yes'
                class='btn iframe' title="确认"
                data-app="qa"><i class='icon-bug-confirmBug icon-ok'></i></a>
            <a href='/zentao/bug-resolve-5848.html?onlybody=yes'
                class='btn iframe showinonlybody' title="解决"
                data-app="qa"><i class='icon-bug-resolve icon-checked'></i></a>
            <a href='/zentao/bug-edit-5848.html' class='btn ' title="编辑Bug"
                data-app="qa"><i class='icon-common-edit icon-edit'></i></a>
            <a href='/zentao/bug-create-8-0-bugID=5848.html' class='btn ' title="复制Bug"
                data-app="qa"><i class='icon-common-copy icon-copy'></i></a>
        </td>
    </tr>"#.replace('\n', " ");

        assert!(BUG_TR_RE.is_match(&trcontet));
    }

    #[test]
    fn ms_bug_re_match() {
        assert!(MS_BUG_RE.is_match("【MS123456】"));
        assert!(MS_BUG_RE.is_match("【123456】"));
        assert!(!MS_BUG_RE.is_match("123"));
    }

    #[test]
    fn md5_works() {
        assert_eq!(
            md5_digest("92d7ddd2a010c59511dc2905b7e14f64169830088"),
            "2fb9050204516115abc63081902b6005".to_string()
        );

        eprintln!(
            "{}",
            md5_digest(&format!("{}{}", md5_digest("1qaz@WSX"), "893725964"))
        );
    }
}

// curl 'http://10.170.136.67:8080/zentao/user-refreshRandom.html?tid=0nkkhq04' \
//   -H 'Accept: */*' \
//   -H 'Accept-Language: zh-CN,zh;q=0.9' \
//   -H 'Cache-Control: no-cache' \
//   -H 'Cookie: zentaosid=woshiyigetoken1; lang=zh-cn; device=desktop; theme=default; tab=my; windowWidth=1132; windowHeight=970' \
//   -H 'Pragma: no-cache' \
//   -H 'Proxy-Connection: keep-alive' \
//   -H 'Referer: http://10.170.136.67:8080/zentao/user-login.html?tid=0nkkhq04' \
//   -H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36' \
//   -H 'X-Requested-With: XMLHttpRequest' \
//   --compressed \
//   --insecure

//   curl 'http://10.170.136.67:8080/zentao/user-login.html?tid=0nkkhq04' \
//   -H 'Accept: application/json, text/javascript, */*; q=0.01' \
//   -H 'Accept-Language: zh-CN,zh;q=0.9' \
//   -H 'Cache-Control: no-cache' \
//   -H 'Content-Type: application/x-www-form-urlencoded; charset=UTF-8' \
//   -H 'Cookie: zentaosid=woshiyigetoken1; lang=zh-cn; device=desktop; theme=default; tab=my; windowWidth=1132; windowHeight=970' \
//   -H 'Origin: http://10.170.136.67:8080' \
//   -H 'Pragma: no-cache' \
//   -H 'Proxy-Connection: keep-alive' \
//   -H 'Referer: http://10.170.136.67:8080/zentao/user-login-L3plbnRhby8=.html?tid=0nkkhq04' \
//   -H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36' \
//   -H 'X-Requested-With: XMLHttpRequest' \
//   --data-raw 'account=yuhao&password=e8183af060a24fe7de5b4651e6912c4e&passwordStrength=1&referer=%2Fzentao%2F&verifyRand=893725964&keepLogin=0&captcha=' \
//   --compressed \
//   --insecure
