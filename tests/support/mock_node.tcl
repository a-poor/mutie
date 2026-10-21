# macOS's installed interpreter avoids executing a freshly linked Mach-O image.
# This private Tcl initialization script runs before the shell reads stdin.
# It is an identity-only stand-in: no JavaScript or Vitest code is executed.
proc mock_node {} {
    global argv
    if {$argv eq [list --fixture-ready]} {
        return
    }
    set expected {console.log(JSON.stringify({version:process.version,executable:process.execPath}))}
    if {[llength $argv] != 2 || [lindex $argv 0] ne "--eval" || [lindex $argv 1] ne $expected} {
        error "identity-only Node fixture must not execute Vitest or another expression"
    }
    # Tcl retains the invoked symlink path, including PATH resolution and spaces.
    set response [open "[file rootname [info nameofexecutable]].response" r]
    fconfigure $response -encoding utf-8 -translation lf
    if {[gets $response counter] < 0} {
        close $response
        error "missing fixture response header"
    }
    set identity [read $response]
    close $response
    if {$counter ne ""} {
        set calls [open $counter a]
        fconfigure $calls -encoding utf-8 -translation lf
        puts $calls called
        close $calls
    }
    puts $identity
}
if {[catch {mock_node} failure]} {
    puts stderr $failure
    exit 1
}
exit 0
