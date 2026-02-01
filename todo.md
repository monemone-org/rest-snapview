

## Outstanding


## Done

x add vi like navigation support

  Page Forward One Screen Ctrl-F. 
  Scroll Forward One-Half Screen Ctrl-D.
  Page Backward One Screen Ctrl-B.
  Scroll Backward One-Half Screen  Ctrl-U.


x enter on a snapshot doesn't show loading and spinning UI
x navigating to a different dir doesn't show loading and spinning UI

x in download file picker, 
    - add .. to go parent dir
    - [Enter] on drive should expand it.
    - add [Download] button to download
    - add [Cancel] button to cancel

x Cache folder result along navigation stack
    e.g. from /A to /A/B
        should cache A result in stack
        from /A/B to /A/B/C
        should cache B result in stack
        from /A/B/C back to /A/B
        should disgard C result , pop B result from stack

x "/" filter:
    - enter  should "select" the selected folder. 
        e.g. if on "> .." , go out a level
        e.g. if on "> subfolder" , go into subfolder

x add a restic command logging pane. about 1/5 of the UI height space. make it scrollable.
it always auto-scroll to show the latest restic command.
    

