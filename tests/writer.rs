use openair::{Airspace, Altitude, Class, Coord, Geometry, PolygonSegment};

#[test]
fn write_single_airspace() {
    let airspace = Airspace {
        name: "Test Zone".to_string(),
        class: Class::D,
        type_: None,
        lower_bound: Altitude::Gnd,
        upper_bound: Altitude::FlightLevel(100),
        geom: Geometry::Circle {
            centerpoint: Coord {
                lat: 47.0,
                lng: 8.0,
            },
            radius: 5.0,
        },
        frequency: None,
        call_sign: None,
        transponder_code: None,
        activation_times: None,
    };

    let mut buf = Vec::new();
    openair::write(&mut buf, [&airspace]).unwrap();
    let output = String::from_utf8(buf).unwrap();

    insta::assert_snapshot!(output, @r"
    AC D
    AN Test Zone
    AL GND
    AH FL100
    V X=47:00:00 N 008:00:00 E
    DC 5
    ");
}

#[test]
fn write_multiple_airspaces() {
    let airspace1 = Airspace {
        name: "Zone A".to_string(),
        class: Class::A,
        type_: None,
        lower_bound: Altitude::Gnd,
        upper_bound: Altitude::FlightLevel(100),
        geom: Geometry::Circle {
            centerpoint: Coord {
                lat: 47.0,
                lng: 8.0,
            },
            radius: 5.0,
        },
        frequency: None,
        call_sign: None,
        transponder_code: None,
        activation_times: None,
    };

    let airspace2 = Airspace {
        name: "Zone B".to_string(),
        class: Class::B,
        type_: None,
        lower_bound: Altitude::FeetAmsl(1000),
        upper_bound: Altitude::FeetAmsl(5000),
        geom: Geometry::Polygon {
            segments: vec![
                PolygonSegment::Point(Coord {
                    lat: 46.0,
                    lng: 7.0,
                }),
                PolygonSegment::Point(Coord {
                    lat: 46.0,
                    lng: 8.0,
                }),
                PolygonSegment::Point(Coord {
                    lat: 45.0,
                    lng: 8.0,
                }),
            ],
        },
        frequency: None,
        call_sign: None,
        transponder_code: None,
        activation_times: None,
    };

    let airspace3 = Airspace {
        name: "Zone C".to_string(),
        class: Class::C,
        type_: Some("CTR".to_string()),
        lower_bound: Altitude::Gnd,
        upper_bound: Altitude::Unlimited,
        geom: Geometry::Circle {
            centerpoint: Coord {
                lat: 48.0,
                lng: 9.0,
            },
            radius: 10.0,
        },
        frequency: Some("123.45".to_string()),
        call_sign: Some("TOWER".to_string()),
        transponder_code: None,
        activation_times: None,
    };

    let mut buf = Vec::new();
    openair::write(&mut buf, [&airspace1, &airspace2, &airspace3]).unwrap();
    let output = String::from_utf8(buf).unwrap();

    insta::assert_snapshot!(output, @r"
    AC A
    AN Zone A
    AL GND
    AH FL100
    V X=47:00:00 N 008:00:00 E
    DC 5

    AC B
    AN Zone B
    AL 1000ft AMSL
    AH 5000ft AMSL
    DP 46:00:00 N 007:00:00 E
    DP 46:00:00 N 008:00:00 E
    DP 45:00:00 N 008:00:00 E

    AC C
    AY CTR
    AN Zone C
    AL GND
    AH UNLIM
    AF 123.45
    AG TOWER
    V X=48:00:00 N 009:00:00 E
    DC 10
    ");
}

#[test]
fn write_empty_iterator() {
    let airspaces: Vec<&Airspace> = vec![];

    let mut buf = Vec::new();
    openair::write(&mut buf, airspaces).unwrap();
    let output = String::from_utf8(buf).unwrap();

    assert_eq!(output, "");
}

#[test]
fn write_with_vec() {
    let airspace = Airspace {
        name: "Test".to_string(),
        class: Class::D,
        type_: None,
        lower_bound: Altitude::Gnd,
        upper_bound: Altitude::FlightLevel(50),
        geom: Geometry::Circle {
            centerpoint: Coord {
                lat: 47.0,
                lng: 8.0,
            },
            radius: 3.0,
        },
        frequency: None,
        call_sign: None,
        transponder_code: None,
        activation_times: None,
    };

    // Test with borrowed Vec
    let airspaces = vec![&airspace];
    let mut buf = Vec::new();
    openair::write(&mut buf, airspaces).unwrap();
    let output = String::from_utf8(buf).unwrap();

    insta::assert_snapshot!(output, @r"
    AC D
    AN Test
    AL GND
    AH FL50
    V X=47:00:00 N 008:00:00 E
    DC 3
    ");
}
