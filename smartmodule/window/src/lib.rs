use std::{sync::OnceLock, marker::PhantomData};

use serde::{Deserialize, Serialize};

use fluvio_smartmodule::{
    smartmodule, Record, Result, eyre,
    dataplane::smartmodule::{SmartModuleExtraParams},
    RecordData,
};

#[derive(Debug, Default)]
struct TumblingWindow<K, V, S> {
    phantom: PhantomData<K>,
    phantom2: PhantomData<V>,
    phantom3: PhantomData<S>,
}

impl<K, V, S> TumblingWindow<K, V, S>
where
    S: Default,
{
    fn new() -> Self {
        Self {
            phantom: PhantomData,
            phantom2: PhantomData,
            phantom3: PhantomData,
        }
    }

    fn add(&self, _key: K, _value: &V) {}

    fn get_state(&self, _key: K) -> Option<S> {
        Some(S::default())
    }
}

type Key = u16;
type DefaultWindowState = TumblingWindow<Key, VehiclePosition, VehicleStatistics>;

#[derive(Debug, Deserialize)]
struct MQTTEvent {
    mqtt_topic: String,
    payload: Payload,
}

#[derive(Debug, Deserialize)]
struct Payload {
    VP: VehiclePosition,
}

/// city of Helinski metro event
/// https://digitransit.fi/en/developers/apis/4-realtime-api/vehicle-positions/high-frequency-positioning/
///
#[derive(Debug, Deserialize)]
struct VehiclePosition {
    desi: String,  // Route number visible to passengers.
    lat: f32,      // WGS 84 latitude in degrees. null if location is unavailable.
    long: f32,     // WGS 84 longitude in degrees.null if location is unavailable.
    dir: String, // Route direction of the trip. After type conversion matches direction_id in GTFS and the topic. Either "1" or "2".
    oper: u16, // Unique ID of the operator running the trip (i.e. this value can be different than the operator ID in the topic, for example if the service has been subcontracted to another operator). The unique ID does not have prefix zeroes here.
    veh: Key, // Vehicle number that can be seen painted on the side of the vehicle, often next to the front door. Different operators may use overlapping vehicle numbers. Matches vehicle_number in the topic except without the prefix zeroes.
    tst: String, // UTC timestamp with millisecond precision from the vehicle in ISO 8601 format (yyyy-MM-dd'T'HH:mm:ss.SSSZ).
    tsi: u64,    // Unix time in seconds from the vehicle.
    spd: f32,    // Speed of the vehicle, in meters per second (m/s).
    hdg: u16, // Heading of the vehicle, in degrees (⁰) starting clockwise from geographic north. Valid values are on the closed interval [0, 360].
    acc: f32, // Acceleration (m/s^2), calculated from the speed on this and the previous message. Negative values indicate that the speed of the vehicle is decreasing.
    dl: u16, // Offset from the scheduled timetable in seconds (s). Negative values indicate lagging behind the schedule, positive values running ahead of schedule.
    odo: u32, // The odometer reading in meters (m) since the start of the trip. Currently the values not very reliable.
    drst: u32, // Door status. 0 if all the doors are closed. 1 if any of the doors are open.
    oday: String, // Operating day of the trip. The exact time when an operating day ends depends on the route. For most routes, the operating day ends at 4:30 AM on the next day. In that case, for example, the final moment of the operating day "2018-04-05" would be at 2018-04-06T04:30 local time.
    jrn: u32,     // Internal journey descriptor, not meant to be useful for external use.
    line: u16,    // Internal line descriptor, not meant to be useful for external use.
    start: String, // Scheduled start time of the trip, i.e. the scheduled departure time from the first stop of the trip. The format follows HH:mm in 24-hour local time, not the 30-hour overlapping operating days present in GTFS. Matches start_time in the topic.
    loc: String, // Location source, either GPS, ODO, MAN, DR or N/A. GPS - location is received from GPS  ODO - location is calculated based on odometer value   MAN - location is specified manually   DR - location is calculated using dead reckoning (used in tunnels and other locations without GPS signal) N/A - location is unavailable
    stop: u32, // ID of the stop related to the event (e.g. ID of the stop where the vehicle departed from in case of dep event or the stop where the vehicle currently is in case of vp event).null if the event is not related to any stop.
    route: String, // ID of the route the vehicle is currently running on. Matches route_id in the topic.
    occu: u16, // Integer describing passenger occupancy level of the vehicle. Valid values are on interval [0, 100]. Currently passenger occupancy level is only available for Suomenlinna ferries as a proof-of-concept. The value will be available shortly after departure when the ferry operator has registered passenger count for the journey.For other vehicles, currently only values used are 0 (= vehicle has space and is accepting passengers) and 100 (= vehicle is full and might not accept passengers)
}

impl VehiclePosition {}

#[derive(Debug, Serialize)]
struct VehicleStatistics {
    vehicle: u16,
    avg_speed: f64, // average speed of vehicle
}

impl Default for VehicleStatistics {
    fn default() -> Self {
        Self {
            vehicle: 22,
            avg_speed: 3.2,
        }
    }
}

impl VehicleStatistics {
    fn add(&mut self, _key: Key, value: VehicleStatistics) {
        self.avg_speed = (self.avg_speed + value.avg_speed) / 2.0;
    }
}

static STATE: OnceLock<DefaultWindowState> = OnceLock::new();

#[smartmodule(init)]
fn init(_params: SmartModuleExtraParams) -> Result<()> {
    STATE
        .set(TumblingWindow::new())
        .map_err(|err| eyre!("state init: {:#?}", err))
}

#[smartmodule(filter_map)]
pub fn filter_map(record: &Record) -> Result<Option<(Option<RecordData>, RecordData)>> {
    let mqtt: MQTTEvent = serde_json::from_slice(record.value.as_ref())?;
    let event = mqtt.payload.VP;

    // for now emit same event

    let key = event.veh.to_string();

    // add to state
    let stats = STATE.get().unwrap();
    stats.add(event.veh, &event);

    // get state
    if let Some(state) = stats.get_state(event.veh) {
        let value_out = serde_json::to_string(&state)?;
        Ok(Some((Some(key.into()), RecordData::from(value_out))))
    } else {
        Ok(None)
    }

    /*
    let value_out = serde_json::to_string(&value)?;

    Ok(Some((Some(key.into()), RecordData::from(value_out))))
    */
}

#[cfg(test)]
mod test {
    use std::fs;

    use super::MQTTEvent;

    #[test]
    fn json_parse() {
        let bytes = fs::read("test/test.json").expect("read file");
        let mqtt: MQTTEvent = serde_json::from_slice(&bytes).expect("parse json");
        let event = mqtt.payload.VP;
        assert_eq!(event.veh, 116);
        assert_eq!(event.lat, 60.178622);
        assert_eq!(event.long, 24.950366);
    }
}
