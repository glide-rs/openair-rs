#![cfg(feature = "serde")]

use openair::*;
use serde_json::to_string;

#[test]
fn serialize_json() {
    let airspace = Airspace {
        name: "SUPERSPACE".into(),
        class: Class::Prohibited,
        lower_bound: Altitude::Gnd,
        upper_bound: Altitude::FeetAgl(3000),
        geom: Geometry::Polygon {
            segments: vec![
                PolygonSegment::Point(Coord { lat: 1.0, lng: 2.0 }),
                PolygonSegment::Point(Coord { lat: 1.1, lng: 2.0 }),
                PolygonSegment::Arc(Arc {
                    centerpoint: Coord {
                        lat: 1.05,
                        lng: 2.05,
                    },
                    start: Coord { lat: 1.1, lng: 2.0 },
                    end: Coord { lat: 1.0, lng: 2.1 },
                    direction: Direction::Cw,
                }),
                PolygonSegment::ArcSegment(ArcSegment {
                    centerpoint: Coord { lat: 3.0, lng: 3.0 },
                    radius: 1.5,
                    angle_start: 30.0,
                    angle_end: 45.0,
                    direction: Direction::Ccw,
                }),
                PolygonSegment::Point(Coord { lat: 1.0, lng: 2.0 }),
            ],
        },
        type_: None,
        frequency: None,
        call_sign: None,
        transponder_code: None,
    };
    assert_eq!(
        to_string(&airspace).unwrap(),
        "{\"name\":\"SUPERSPACE\",\
              \"class\":\"Prohibited\",\
              \"lowerBound\":{\"type\":\"Gnd\"},\
              \"upperBound\":{\"type\":\"FeetAgl\",\"val\":3000},\
              \"geom\":{\
                \"type\":\"Polygon\",\
                \"segments\":[\
                  {\"type\":\"Point\",\"lat\":1.0,\"lng\":2.0},\
                  {\"type\":\"Point\",\"lat\":1.1,\"lng\":2.0},\
                  {\"type\":\"Arc\",\
                   \"centerpoint\":{\"lat\":1.05,\"lng\":2.05},\
                   \"start\":{\"lat\":1.1,\"lng\":2.0},\
                   \"end\":{\"lat\":1.0,\"lng\":2.1},\
                   \"direction\":\"cw\"},\
                  {\"type\":\"ArcSegment\",\
                   \"centerpoint\":{\"lat\":3.0,\"lng\":3.0},\
                   \"radius\":1.5,\
                   \"angleStart\":30.0,\
                   \"angleEnd\":45.0,\
                   \"direction\":\"ccw\"},\
                  {\"type\":\"Point\",\"lat\":1.0,\"lng\":2.0}\
                ]\
              }\
             }"
    );
}

#[test]
fn serialize_json_ctr() {
    let airspace = Airspace {
        name: "Control Zone".into(),
        class: Class::Ctr,
        lower_bound: Altitude::Gnd,
        upper_bound: Altitude::FeetAgl(1000),
        geom: Geometry::Polygon { segments: vec![] },
        type_: None,
        frequency: None,
        call_sign: None,
        transponder_code: None,
    };
    assert_eq!(
        to_string(&airspace).unwrap(),
        "{\"name\":\"Control Zone\",\
              \"class\":\"CTR\",\
              \"lowerBound\":{\"type\":\"Gnd\"},\
              \"upperBound\":{\"type\":\"FeetAgl\",\"val\":1000},\
              \"geom\":{\
                \"type\":\"Polygon\",\
                \"segments\":[]\
              }\
             }"
    );
}
