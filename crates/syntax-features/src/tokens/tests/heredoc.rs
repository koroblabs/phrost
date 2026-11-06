//! A heredoc, which is one token spanning many lines.
//!
//! The shape that breaks a highlighter rather than dimming it: a client is sent absolute ranges and
//! turns each into a line and a column itself, so a token that crosses a line boundary is the one
//! case where being wrong about a range is visible as *the rest of the file* losing its colour.
//!
//! And the one string whose contents are routinely another language. `<<<TWIG` is written to get Twig
//! highlighting, so the body belongs to whatever grammar the client injects for it and the delimiters
//! belong to us -- a flat string token per line is what takes the tags away.

use expect_test::expect;

use super::check;

#[test]
fn a_heredocs_body_is_left_to_the_clients_own_grammar() {
    check(
        r#"<?php

namespace App;

class Mail
{
    protected string $template = <<<TWIG
        <div style="padding: 0 1rem;">
            <p>Hello, #{{ name }}</p>
        </div>
        TWIG;

    public function after(): string
    {
        return $this->template;
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      namespace        keyword    
            3:10     App              namespace  
            5:0      class            keyword    
            5:6      Mail             class      declaration
            7:4      protected        keyword    
            7:14     string           type       
            7:21     $template        property   declaration
            7:33     <<<TWIG          string     
            11:0             TWIG     string     
            13:4     public           keyword    
            13:11    function         keyword    
            13:20    after            method     declaration
            13:29    string           type       
            15:8     return           keyword    
            15:15    $this            keyword    
            15:22    template         property   
        "#]],
    );
}

#[test]
fn an_interpolated_string_is_still_a_string_and_its_variables_are_variables() {
    // A heredoc's body and a double-quoted string's chunks come back from the lexer as the same kind,
    // so the rule about one must not reach the other. And a `$name` inside a heredoc *is* PHP -- the
    // language interpolates it, so it is coloured wherever it is written.
    check(
        r#"<?php

$greeting = "Hello, $name and $other";

$sql = <<<SQL
    SELECT * FROM t WHERE id = $id
    SQL;
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      $greeting        variable   
            3:13     Hello,           string     
            3:20     $name            variable   
            3:25      and             string     
            3:30     $other           variable   
            5:0      $sql             variable   
            5:7      <<<SQL           string     
            6:31     $id              variable   
            7:0          SQL          string     
        "#]],
    );
}
