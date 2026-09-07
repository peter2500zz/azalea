use azalea_brigadier::{builder::argument_builder::ArgumentBuilder, prelude::*};

#[test]
fn test_arguments() {
    let builder: ArgumentBuilder<()> = literal("foo");

    let argument = integer::<(), i32>("bar");
    let builder = builder.then(argument.clone());
    assert_eq!(builder.children().len(), 1);
    let built_argument = argument.build();
    assert!(builder.children().contains(&built_argument));
}

/// `describe` is just another builder setter, so it has to survive the same
/// clone-then-build path as `executes` and friends.
#[test]
fn test_describe() {
    let builder: ArgumentBuilder<()> = literal("foo").describe("does the foo thing");

    assert_eq!(
        builder.clone().build().description.as_deref(),
        Some("does the foo thing")
    );
    // Last word wins, like every other setter here.
    assert_eq!(
        builder
            .describe("does something else")
            .build()
            .description
            .as_deref(),
        Some("does something else")
    );
    assert_eq!(literal::<(), i32>("bar").build().description, None);
}

//     @Test
//     public void testRedirect() throws Exception {
//         final CommandNode<Object> target = mock(CommandNode.class);
//         builder.redirect(target);
//         assertThat(builder.getRedirect(), is(target));
//     }

//     @Test(expected = IllegalStateException.class)
//     public void testRedirect_withChild() throws Exception {
//         final CommandNode<Object> target = mock(CommandNode.class);
//         builder.then(literal("foo"));
//         builder.redirect(target);
//     }

//     @Test(expected = IllegalStateException.class)
//     public void testThen_withRedirect() throws Exception {
//         final CommandNode<Object> target = mock(CommandNode.class);
//         builder.redirect(target);
//         builder.then(literal("foo"));
//     }

//     private static class TestableArgumentBuilder<S> extends
// ArgumentBuilder<S, TestableArgumentBuilder<S>> {         @Override
//         protected TestableArgumentBuilder<S> getThis() {
//             return this;
//         }

//         @Override
//         public CommandNode<S> build() {
//             return null;
//         }
//     }
// }
