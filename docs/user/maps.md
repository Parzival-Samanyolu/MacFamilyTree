# Maps

Open **Map** in the sidebar.

## Showing events
* Choose which event types to show (birth, death, marriage, residence, …). Living people are hidden unless you clear *Hide living people*.
* **Display**: *Markers* (clustered; click a marker to open the person), *Heat map*, *Migration* (birth → death place) or *Lineage* (parent's birth place → child's).
* Tick *Selected person only* to draw that person's route in date order.
* The **From / To** sliders filter by year; **Play** animates the map through time.
* *Show as table* lists the same events for screen readers and keyboard users.

## Getting coordinates
1. **Locate places offline** fills in cities and countries from the built-in gazetteer (about 260 places, including Turkish provinces; aliases such as *Constantinople* and *Türkiye* work).
2. Places without coordinates appear in the side list. Press **Set on map**, then click the map.
3. Optionally allow **online lookup** (Nominatim). Place names are then sent to nominatim.openstreetmap.org, one request per second, and answers are cached on this computer.

A place without its own coordinates is drawn at its parent's (for example a village at its province) and marked *approximate* in the table.

## Base maps
* **Offline world outline** – default, no network.
* **OpenStreetMap (online)** – requests tiles from tile.openstreetmap.org.
* **Offline tiles** – pick a `.pmtiles` file (raster, or vector with OpenMapTiles layer names). Nothing is uploaded.

## Export
GeoJSON and KML (for Google Earth and GIS tools) of the currently filtered events, and a PNG of the map view.
