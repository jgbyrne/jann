## jann

`jann` is a configuration deployment utility for \*nix operating systems.

*Disclaimer: `jann` is alpha software. I implore you not to test it on any system that matters. `jann` is a penknife not a butter knife, and can overwrite important data with ease.*

The idea behind `jann` is a simple one - you put all your configuration files in one directory tree, then write a manifest called a `Jannfile` to specify where in your filesystem those configuration files should be copied to. Note the difference in philosophy to the traditional technique of using `GNU stow`, which relies on softlinks.

A typical `jann` bundle might look something like this:

```
   awesome_config/
       Jannfile
       .bashrc
       .vimrc
       .vim/
           ...
       sway/
           config
           status_config.toml
       wallpaper.png
```

The simple, readable `Jannfile` would then be as follows:

```
   console {
     .bashrc => ~/
     .vimrc => ~/
     .vim => ~/
   }
   
   graphical {
     sway/config => ~/.config/sway/
     sway/status_config.toml => ~/.config/sway/
     wallpaper.png >> ~/pic/wallpaper
   }

   main
     | console
     | graphical
```

The deployment may be completed in one simple command `jann Jannfile`.

Note the two key structures here, **blocks** - named sequences of instructions surrounded by curly braces, and **pipelines** - named sequences of blocks (or other pipelines) to be run consecutively. The default pipeline is `main` - a different entry point can be specified with `--execute <pipeline>`.

Note also the two different types of arrows used to represent two different types of copy operations - insertion copies, where the left path is copied into the right path, and 'splatting' copies, where the left path is copied directly onto the right path.

Something important to note here is that when a directory is copied on top of another folder in `jann`, the original folder is completely deleted. This is a deliberate choice, but one which I realise goes against the behaviour of traditional tools and as such could catch the unwary user out. You can prevent any directories being overwritten with the switch `--forbid DD FD` - more on that later. 

This brief example does not cover much of `jann`'s functionality. Here are some examples of other features of `jann`.

**Variables**

`jann` supports local (scoped) and global variables, which can be interpolated into strings and commands.

```
   foo {
     @glob = "Hello"
     loc = "Hiya"
   }

   bar {
     // Will echo Hello
     $ echo {{glob}}

     // Will also echo Hello
     loc2 = @glob
     $ echo {{loc2}}

     // Not gonna work!
     $ echo {{loc}}
   }

   main
     | foo
     | bar
```

**Command Execution**

As indicated in the previous example, it is possible to run arbritrary shell commands.

```
   shell_out {
     msg = "Yo!"
     $ echo {{msg}}
   }
```

**Maps**

Maps allow the same instructions to be performed on a range of values.

```
   colours {
     ["redfile", "bluefile", "greenfile"] -> c {
         "{{c}}" => ~/colourfiles/
     }
   }
```

**Enabling and Disabling**

Stages within pipelines can be, by default, enabled or disabled. Enabled stages are marked with a pipe '`|`', while disabled ones are marked with a colon '`:`'. This is more clear in an example:

```
   my_pipeline
      | fiddle       <-- enabled
      : lacquer      <-- disabled
      | spin         <-- enabled
      : incinerate   <-- disabled
```

The default enable and disable states can be modified with the command line switches `--enable` and `--disable`. These switches take the following arguments.

* `"*"` - Apply to every stage of every pipeline
* `%foo` - Apply to every stage tagged `foo`
* `bar` - Apply to every instance of the stage `bar`
* `spqr.%foo` - Apply to every stage tagged `foo` in the pipeline `spqr`
* `spqr.bar` - Apply to the stage `bar` in the pipleline `spqr`

This might lead you to the natural question - what is a tag? Good question! Tags can be applied to pipeline stages like so:

```
   spqr
     | pillage [important, destructive]
     | frolic
     | encamp [important]
     | barrage [destructive]
```

In this example, `important` and `destructive` are tags.

**Options**

`jann` features a fine-grained options system which allows control over the extent to which your filesystem can be modified.

It features the following flags:

* FF - Files can be overwritten by Files
* DD - Directories can be overwritten by Directories
* DF - Directories can be overwritten by Files
* FD - Files can be overwritten by Directories
* INTER - Intermediate directories can be created to complete a copy

These flags can be turned on and off with the `--allow` and `--forbid` switches. For example:

    jann Jannfile --allow FF DD --disallow DF FD INTER

These chosen options propogate to any auxilliary Jannfiles included with directives (see below).

**Includes**

It is possible to bring references to other Jannfiles into the namespace. This may be desirable for the sake of modularity, or to allow certain instructions to run as root.

Here are some examples of how these directives can be used

```
   // Bring the spqr pipeline of other.Jannfile into the namespace
   # include other.Jannfile::spqr
   // Bring the main (default) pipeline of priv.Jannfile into the namespace as 'elevated'
   # sudo_include [priv.Jannfile, elevated]

   main
     | spqr
     | elevated
```

