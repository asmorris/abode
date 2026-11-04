I want you to help me write an application for home owners.
It should be written in Rust.

What i think it should have for attributes:

1. 2D/3D modelling capabilities for each of the rooms
  - should be able to input the rooms from either an existing blueprint, or your own designs
  - should be able to change the length/widths of the rooms (at least when first input)
  - should be able to have multiple rooms
  - should be able to build a house from all of the rooms
  - should be able to label the rooms
  - should be able to label the walls
  - should be able to add notes to a given room
  - should also have some information for the roof
  - should be able to add notes around paints used
2. In the rooms, there should be the ability to have more granular info put in, with the ability to make objects to place in the rooms (maybe from a bank of primatives such as circles/squares/rectangles etc)
  - these objects should be associated with a given room, and should have their own labels, such as fridge, furnace, washer/dryer etc
  - should be able to input notes for these objects
  - these notes should be able to also have images (possibly many) for manuals, state, repair info etc
  - should have ability to input date serviced, date bought, all pertinent information around their upkeep
3. associated with the rooms there should be information for windows/doors
  - windows and doors should be able to input notes/dates as the other objects are
4. General notes section for all of the general notes you would need


- the file with all of the information (or entire scene if you will) should be shareable (behind a paid service? perhaps free?) so that someone buying the house could simply get the file with all of the information the previous homeowner gave
- files need to be saveable/loadable


maybe
- perhaps a timeline of changes (sort of like git history for different rooms/objects)?
- section for contractors to be able to come into the file in a limited manner and make changes about what they have done

eventual (perhaps for contractors as well)
- wiring information with lights/switches for a user to be able to input with constraints based on the room
- plumbing information with constraints based on the room

