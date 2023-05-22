# fbug: stateful phone debugger

fbug is a daemon which provides a sensible interface for interacting with DUTs
(devices under test). It supports bidirectional serial communication, control of
the serial devices RTS/DTR pins, fastboot (TBD), EDL (TBD: feasibility?) and
TCP/IP (maybe just SSH?).

## Configuration

fbug uses a configuration file per device, configuration files are written in
yaml. A big TODO is to find a way to minimise config duplication without making
things overly complicated..

A config file is made up of a preamble, followed by a few specific sections:

* **Connections**: A list of connections to the DUT and any assiociated config
* **States**: A list of possible device states and how to detect when one is
  entered
* **Controls**: Describes what things fbug can change for this device
* **Transitions**: An exhaustive list of valid state transitions and how they
  can be performed

These allow fbug to expose a generic interface for controlling, debugging, and
generally interacting with a DUT that can be easily scripted and exported for
remote use.

### Top level config

* name: Friendly name of this device
* codename: computer-friendly name
* descriptions: a high level description of the device and setup
* username: The login username for SSH or GETTY
* password: The login password
* resting-state: (default: off) the name of the state this device should enter
  when not in use.

### Connections

This section describes the possible connections to a device (or device-adjacent
peripheral like an external power control board) as well as any relevant
configuration. Note that this section should not describe how these connections
are used, just that they exist.

A connection is a yaml object, where the `type` properties dictates what other
properties are supported. Accepted types are:

* serial
* usb
* ssh

The `label` property is valid for all types, it is a string used to refer to
this connection throughout the rest of the configuration.

Connections also have actions and events associated with them, such as sending
or receiving data, (dis)connecting, adjusting settings (like baud rate),
controlling pins (like RTS/DTR), etc.

All connections implicitly support the connect/disconnect and send/receive
actions.

#### Serial

* path: (required) path to serial device, e.g. `/dev/ttyUSB0`. I would recommend
  using `/dev/serial/by-id` aliases
* baud: (required) The default baud rate, used if a state doesn't override it
* getty: (default: false) Does this port ever spawn a getty

Supported actions are:

* baud: adjust the baud rate
* dtr: set the DTR pin
* rts: set the RTS pin

#### USB

* port: The USB port as shown is `/sys/bus/usb/devices`
  [lsplug](https://git.sr.ht/~martijnbraam/lsplug) is useful to determine this.

#### ssh

SSH by default uses much stricter alive checks, this means it will timeout much
faster if the board hangs up.

* host: (required) the IP or host name
* port: (default: 22) the port to use
* alive_interval: (default: 1) how often to send alive checks
* alive_count_max: (default: 8) how many missed pongs before disconnect

### Controls

A list of objects which each describe a single control for the DUT.

<!-- The following controls are supported

* power (controls the boards primary power supply)
* aux_power (a secondary power supply that must be turned on after the primary
  and off before it)
*  -->

Supported control types are:

* Button
* Switch (alias for button)
* Command

All controls share the same properties:

* connection: (required) the label of the connection this control uses
* action: (required) what action to take on the connection
* values: (optional) the values to send for non-boolean actions must be an array
  of length 2, the first item for ON, the second for OFF

describe transitions in states? nah... "Hung" implicit state?

### States

The states section describes the possible device states and any associated
configuration changes that should be made when entering a state (e.g. changing
the baud rate). The "off", and "unknown" states don't have to be defined. The
state namy "any" is reserved.

* name: (required) the name of the state...
* baud: (required) the baud rate to set while in this state
* ... TBD

### Transitions

The possible state transitions and their triggers. It is an error for a state
transition to happen that isn't explicitly defined here.

* to: (required) the name of the state this transition is to
* from: (required) an array of states it's possible to transition from. This can be null
  (empty) if this transition can occur from **any** other state. Be careful of this!
* actions: (mutually exclusive with timeout) The actions/events that causes this transition.
  * source: (required) The name of the connection
  * event: (required) The event (e.g. input)
  * value: (required) The value of the action/event, strings starting and ending
    with a `/` are treated as PCRE regex.
* timeout: (optional) indicates that this transition occurs if the device is in
  any of the "from" states for longer than the specified time (in seconds)
* trigger: (optional) A list of sequences of controls to perform this state transition
  * from: (the from states this sequence is valid for, or null (empty) for all valid from states)
  * sequence: (The sequence to perform)
    * control: the control to affect (or "wait")
    * action: one of ("on", "off", "press", "release", "hold") press/release are just aliases
      "hold" means that the button should be held until the transition has occured
    * duration: (default: 0) time in ms before going to the next step in the sequence
