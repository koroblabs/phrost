//! The label of a named argument, which no other arm reaches.

use expect_test::expect;

use super::check;

/// The label is the callee's parameter, in a call and in an attribute alike.
///
/// Both spellings in one fixture because the two reach the arm by different paths -- an
/// attribute's arguments are the same node as a call's -- and a change that colours one and not
/// the other would otherwise read as covered.
#[test]
fn a_named_argument_labels_the_parameter_it_fills() {
    check(
        r#"<?php

#[Definition(id: 'account', label: 'Account')]
class Card
{
    public function build(): Card
    {
        return new Card(label: 'Account', sources: [], statuses: []);
    }
}
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:2      Definition       decorator  
            3:13     id               parameter  
            3:17     'account'        string     
            3:28     label            parameter  
            3:35     'Account'        string     
            4:0      class            keyword    
            4:6      Card             class      declaration
            6:4      public           keyword    
            6:11     function         keyword    
            6:20     build            method     declaration
            6:29     Card             class      
            8:8      return           keyword    
            8:15     new              keyword    
            8:19     Card             class      
            8:24     label            parameter  
            8:31     'Account'        string     
            8:42     sources          parameter  
            8:55     statuses         parameter  
        "#]],
    );
}

/// A positional argument has no label, so nothing is emitted where one would be.
///
/// The negative half: the arm reads the *named* node, and a rule that reached every argument
/// would colour the value's first token as a parameter and be invisible in the positive test.
#[test]
fn a_positional_argument_contributes_no_label() {
    check(
        r#"<?php

$card = new Card('account', 'Account');
"#,
        expect![[r#"
            1:0      <?php            keyword    
            3:0      $card            variable   
            3:8      new              keyword    
            3:12     Card             class      
            3:17     'account'        string     
            3:28     'Account'        string     
        "#]],
    );
}
