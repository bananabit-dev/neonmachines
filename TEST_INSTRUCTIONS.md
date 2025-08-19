# Paste and Cursor Test Instructions

## Test Multi-line Paste
1. Copy the following multi-line text:
```
This is a multi-line test
It has multiple lines
And should be pasted as one input
```

2. Paste it into the Neonmachines TUI (usually Ctrl+V or middle-click)
3. Verify that:
   - All text appears in the input field
   - Cursor moves to the end of pasted content
   - No popup dialogs appear
   - The text is treated as a single input

## Test Cursor Movement
1. Enter some multi-line text:
```
Line 1: Hello
Line 2: World
Line 3: Test
```

2. Test arrow key movements:
   - Left/Right arrows: Move cursor within lines
   - Up/Down arrows: Move cursor between lines
   - Verify cursor stays at correct position

3. Test with pasted content:
   - Paste multi-line text
   - Move cursor around with arrow keys
   - Verify cursor tracking is accurate

## Expected Behavior
- Multi-line pastes should not trigger Windows popup dialogs
- Cursor should move accurately between lines
- Pasted content should be treated as a single input
- No message splitting should occur

## Key Features Fixed
- ✅ Proper multi-line paste handling
- ✅ Accurate cursor positioning
- ✅ Cross-platform paste support
- ✅ No popup dialogs on paste
- ✅ Proper line ending normalization
