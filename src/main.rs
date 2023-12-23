use mpd::*;

fn main() {
    let mut client = Client::connect("127.0.0.1:6600").unwrap();
    println!("Status: {:?}", client.status());
    let artists = client
        .list(&Term::Tag("artist".into()), &Query::new())
        .unwrap();
    
    for artist in artists.iter().take(10) {
        let mut query = Query::new();
        query.and(Term::Tag("artist".into()), artist);
        // mpc list album artist 65daysofstatic group date
        let albums = client.list(&Term::Tag("album".into()), &query).unwrap();
        println!("{} => {:?}", artist, albums);
        if let Some(album) = albums.first() {
            // mpc find artist Floex album Zorya
            let mut query = Query::new();
            query.and(Term::Tag("artist".into()), artist);
            query.and(Term::Tag("album".into()), album);
            let songs = client.find(&query, None).unwrap();
            for song in songs.iter() {
                println!("  {:?}", song.file);
            }
            println!("{:?}", client.albumart(songs.first().unwrap()).unwrap().len());
        }
    }

}
