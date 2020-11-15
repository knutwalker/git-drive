# git_drive


Support for switching git authors and co-authors

## Usage

```bash
# Prompt for a navigator / co-author, or a list thereof, and prepare a new drive
git drive

# Start driving with the specified navigator(s)
git drive with user1 [user2...]

# Start driving alone
git drive alone

# List known navigators
git drive list

# Edit navigator(s), either prompted for, or specified
git drive edit [user1 [user2...]]

# Add new navigator. Values not provided will be prompted
git drive new [[--as] user --name User --email Email]

# Delets navigator(s), either prompted for, or specified
git drive delete [user1 [user2...]]

# List known aliases for the driver
git drive me list
git driver list

# Edit driver, either prompted for, or specified
git drive me edit [user1 [user2...]]
git driver edit [user1 [user2...]]

# Add new driver. Values not provided will be prompted
git drive me new [[--as] user --name User --email Email]
git driver add [[--as] user --name User --email Email]

# Delets a driver, either prompted for, or specified
git drive me delete [user1 [user2...]]
git driver delete [user1 [user2...]]
```


License: MIT OR Apache-2.0
