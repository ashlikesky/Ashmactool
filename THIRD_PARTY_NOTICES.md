# Third-party origins and notices

Reference: https://github.com/sdmj76/Mousecape-swiftUI
Inspected revision: 6e5f4f3ee3ff8f38a8d1516c49a8012e760235d4
Original project: https://github.com/alexzielenski/Mousecape

This local Rust implementation refers to the cursor-registration mechanism and
C API signatures documented by Alex Zielenski and CGSInternal. It is not the
Mousecape application and includes no upstream SwiftUI UI, Swift CLI/helper,
Windows conversion implementation, or sample cursor themes. The upstream
Section B software is kept only in the local work/reference-mousecape checkout
for inspection, and is not part of the app or source package.

The original notices are preserved below. New Rust code is visibly distinct
from the upstream Objective-C/Swift implementation. This build is supplied for
personal non-commercial use. Apple private API availability is not guaranteed.

## Mousecape original work notice (Section A)

==============================================================================

Copyright (c) 2013-2014 Alex Zielenski. All rights reserved.

Applies to: Objective-C model layer, private API layer (mousecloak/), and all
original code inherited from https://github.com/alexzielenski/Mousecape.

Redistribution and use in source and binary forms, with or without modification,
are permitted provided that the following conditions are met:

    * Redistributions of source code must retain the above copyright notice,
      this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above copyright notice,
      this list of conditions and the following disclaimer in the documentation
      and/or other materials provided with the distribution.
    * Any redistribution, use, or modification is done solely for personal
      benefit and not for any commercial purpose or for monetary gain.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDER "AS IS" AND ANY EXPRESS OR
IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT
SHALL THE COPYRIGHT HOLDER BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
OF SUCH DAMAGE.

==============================================================================

## CGSInternal cursor header notice

The Rust FFI signatures are re-declared for Rust, not a verbatim source copy.
Original declaration header notice:

```text
/*
 * Copyright (C) 2007-2008 Alacatia Labs. (C) 2011-2012 Alex Zielenski
 * 
 * This software is provided 'as-is', without any express or implied
 * warranty.  In no event will the authors be held liable for any damages
 * arising from the use of this software.
 * 
 * Permission is granted to anyone to use this software for any purpose,
 * including commercial applications, and to alter it and redistribute it
 * freely, subject to the following restrictions:
 * 
 * 1. The origin of this software must not be misrepresented; you must not
 *    claim that you wrote the original software. If you use this software
 *    in a product, an acknowledgment in the product documentation would be
 *    appreciated but is not required.
 * 2. Altered source versions must be plainly marked as such, and must not be
 *    misrepresented as being the original software.
 * 3. This notice may not be removed or altered from any source distribution.
 * 
 * Joe Ranieri    joe@alacatia.com
 * Alex Zielenski alex@alexzielenski.com
 *
 */
```

## Rust dependencies

The app links the macOS system frameworks and the following Rust dependencies:
objc2 (MIT), objc2-encode (MIT), objc2-foundation (MIT),
objc2-core-foundation (MIT), and bitflags (MIT OR Apache-2.0).
Versions are pinned in Cargo.lock. The build script copies the preserved notices under licenses/ into the
application resources when packaging. The objc2 project MIT license covers
objc2, objc2-encode, and the generated framework crates under the selected MIT
option. The upstream license source is
https://github.com/madsmtm/objc2/blob/main/LICENSE-MIT.txt.
