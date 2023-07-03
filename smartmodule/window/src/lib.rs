use std::sync::OnceLock;

use serde::Deserialize;

use fluvio_smartmodule::{
    smartmodule, Record, Result, eyre,
    dataplane::smartmodule::{SmartModuleExtraParams, SmartModuleInitError},
    RecordData,
};

#[derive(Debug)]
struct State {}

impl State {
    fn new() -> Self {
        Self {}
    }
}

/// city of Helinski metro event
/// https://digitransit.fi/en/developers/apis/4-realtime-api/vehicle-positions/high-frequency-positioning/
///
#[derive(Debug, Deserialize)]
struct VehiclePosition {
    desi: String,  // Route number visible to passengers.
    dir: String, // Route direction of the trip. After type conversion matches direction_id in GTFS and the topic. Either "1" or "2".
    oper: u16, // Unique ID of the operator running the trip (i.e. this value can be different than the operator ID in the topic, for example if the service has been subcontracted to another operator). The unique ID does not have prefix zeroes here.
    veh: u16, // Vehicle number that can be seen painted on the side of the vehicle, often next to the front door. Different operators may use overlapping vehicle numbers. Matches vehicle_number in the topic except without the prefix zeroes.
    tst: String, // UTC timestamp with millisecond precision from the vehicle in ISO 8601 format (yyyy-MM-dd'T'HH:mm:ss.SSSZ).
    tsi: String, // Unix time in seconds from the vehicle.
    spd: f32,    // Speed of the vehicle, in meters per second (m/s).
    hdg: u16, // Heading of the vehicle, in degrees (⁰) starting clockwise from geographic north. Valid values are on the closed interval [0, 360].
    lat: f32, // WGS 84 latitude in degrees. null if location is unavailable.
    long: f32, // WGS 84 longitude in degrees.null if location is unavailable.
    acc: f32, // Acceleration (m/s^2), calculated from the speed on this and the previous message. Negative values indicate that the speed of the vehicle is decreasing.
    dl: u16, // Offset from the scheduled timetable in seconds (s). Negative values indicate lagging behind the schedule, positive values running ahead of schedule.
    odo: u32, // The odometer reading in meters (m) since the start of the trip. Currently the values not very reliable.
    drst: u32, // Door status. 0 if all the doors are closed. 1 if any of the doors are open.
    oday: String, // Operating day of the trip. The exact time when an operating day ends depends on the route. For most routes, the operating day ends at 4:30 AM on the next day. In that case, for example, the final moment of the operating day "2018-04-05" would be at 2018-04-06T04:30 local time.
    jrn: u32,     // Internal journey descriptor, not meant to be useful for external use.
    line: u16,    // Internal line descriptor, not meant to be useful for external use.
    start: String, // Scheduled start time of the trip, i.e. the scheduled departure time from the first stop of the trip. The format follows HH:mm in 24-hour local time, not the 30-hour overlapping operating days present in GTFS. Matches start_time in the topic.
    loc: String, // Location source, either GPS, ODO, MAN, DR or N/A. GPS - location is received from GPS  ODO - location is calculated based on odometer value   MAN - location is specified manually   DR - location is calculated using dead reckoning (used in tunnels and other locations without GPS signal) N/A - location is unavailable
    stop: String, // ID of the stop related to the event (e.g. ID of the stop where the vehicle departed from in case of dep event or the stop where the vehicle currently is in case of vp event).null if the event is not related to any stop.
    route: String, // ID of the route the vehicle is currently running on. Matches route_id in the topic.
    occu: u16, // Integer describing passenger occupancy level of the vehicle. Valid values are on interval [0, 100]. Currently passenger occupancy level is only available for Suomenlinna ferries as a proof-of-concept. The value will be available shortly after departure when the ferry operator has registered passenger count for the journey.For other vehicles, currently only values used are 0 (= vehicle has space and is accepting passengers) and 100 (= vehicle is full and might not accept passengers)
}

static REGEX: OnceLock<State> = OnceLock::new();

#[smartmodule(init)]
fn init(params: SmartModuleExtraParams) -> Result<()> {
    REGEX
        .set(State::new())
        .map_err(|err| eyre!("regex init: {:#?}", err))
}

/*
#[smartmodule(filter)]
pub fn filter(record: &Record) -> Result<bool> {
    let string = std::str::from_utf8(record.value.as_ref())?;
    Ok(REGEX.get().unwrap().is_match(string))
}
*/

#[smartmodule(filter_map)]
pub fn filter_map(record: &Record) -> Result<Option<(Option<RecordData>, RecordData)>> {
    let event: VehiclePosition = serde_json::from_slice(record.value.as_ref())?;

    let key = record.key.clone();
    let string = String::from_utf8_lossy(record.value.as_ref()).to_string();
    let int: i32 = string.parse()?;

    if int % 2 == 0 {
        let output = int / 2;
        Ok(Some((key.clone(), RecordData::from(output.to_string()))))
    } else {
        Ok(None)
    }
}
