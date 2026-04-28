pub fn clean_html(html: &str) -> String {
    const LT_PLACEHOLDER: &str = "\x01";
    const GT_PLACEHOLDER: &str = "\x02";

    let text = html
        .replace("&#x2019;", "'")
        .replace("&#x2018;", "'")
        .replace("&#x201C;", "\"")
        .replace("&#x201D;", "\"")
        .replace("&#x2013;", "-")
        .replace("&#x2014;", "\u{2014}")
        .replace("&amp;", "&")
        .replace("&lt;", LT_PLACEHOLDER)
        .replace("&gt;", GT_PLACEHOLDER)
        .replace("&quot;", "\"")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</li>", "\n")
        .replace("</ul>", "\n")
        .replace("</ol>", "\n");

    let mut result = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            result.push(c);
        }
    }

    result = result
        .replace(LT_PLACEHOLDER, "<")
        .replace(GT_PLACEHOLDER, ">");

    result
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_html_entities() {
        assert_eq!(clean_html("&amp;"), "&");
        assert_eq!(clean_html("&quot;"), "\"");
        assert_eq!(clean_html("a &lt;b&gt; c"), "a <b> c");
    }

    #[test]
    fn test_clean_html_smart_quotes() {
        assert_eq!(clean_html("&#x2019;hello&#x2018;"), "'hello'");
        assert_eq!(clean_html("&#x201C;hi&#x201D;"), "\"hi\"");
    }

    #[test]
    fn test_clean_html_dashes() {
        assert_eq!(clean_html("&#x2013;"), "-");
        assert_eq!(clean_html("&#x2014;"), "\u{2014}");
    }

    #[test]
    fn test_clean_html_strips_tags() {
        assert_eq!(clean_html("<b>bold</b> <i>italic</i>"), "bold italic");
    }

    #[test]
    fn test_clean_html_br_to_newline() {
        assert_eq!(clean_html("line1<br>line2"), "line1\nline2");
        assert_eq!(clean_html("line1<br/>line2"), "line1\nline2");
        assert_eq!(clean_html("line1<br />line2"), "line1\nline2");
    }

    #[test]
    fn test_clean_html_block_elements_to_newline() {
        assert_eq!(clean_html("<p>para1</p><p>para2</p>"), "para1\npara2");
    }

    #[test]
    fn test_clean_html_trims_lines() {
        assert_eq!(clean_html("  hello  \n  world  "), "hello\nworld");
    }

    #[test]
    fn test_clean_html_removes_empty_lines() {
        assert_eq!(clean_html("hello\n\n\nworld"), "hello\nworld");
    }

    #[test]
    fn test_clean_html_plain_text_unchanged() {
        assert_eq!(clean_html("hello world"), "hello world");
    }

    #[test]
    fn test_clean_html_empty_input() {
        assert_eq!(clean_html(""), "");
    }
}
